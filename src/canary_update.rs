use crate::{MANAGE_CHECK, LogResult};
use std::{
	sync::{Arc, Mutex},
	time::{Duration, Instant},
	ops::Deref,
	thread
};
use serenity::{
	model::prelude::*,
	utils::MessageBuilder,
	prelude::*,
	framework::standard::{*,macros::{command, group}}
};
use sqlite::*;
#[group]
#[commands(setupdatechannel, getupdatechannel, unsetupdatechannel)]
#[description = "Commands related to canary updates"]
struct CanaryUpdateCommands;

#[command]
#[checks(Manage)]
#[description = "Sets the channel for updates"]
#[only_in(guilds)]
fn setupdatechannel(ctx: &mut Context, msg: &Message) -> CommandResult {
	if let Some(guild_id) = msg.guild_id {
		let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
		let lock = canary.lock().unwrap();
		
		let res = lock.set_update_channel(&guild_id, &msg.channel_id);
		if res.is_ok() {
			msg.channel_id.say(&ctx, "Successfully set this channel to the canary update notification channel").log_err();
		}
		else {
			msg.channel_id.say(&ctx, "Sql bad").log_err();
			res.log_err();
		}
	}
	else {
		msg.channel_id.say(&ctx, "Well how tf did this happen").log_err();
	}
	Ok(())
}
#[command]
#[description = "Gets the channel for updates"]
#[only_in(guilds)]
fn getupdatechannel(ctx: &mut Context, msg: &Message) -> CommandResult {
	if let Some(guild_id) = msg.guild_id {
		let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
		let lock = canary.lock().unwrap();
		
		let res = lock.get_update_channel(&guild_id);
		if let Some(id) = res {
			msg.channel_id.say(&ctx, MessageBuilder::new().channel(id)).log_err();
		}
		else {
			msg.channel_id.say(&ctx, "None").log_err();
		}
	}
	else {
		msg.channel_id.say(&ctx, "Well how tf did this happen").log_err();
	}
	Ok(())
}

#[command]
#[checks(Manage)]
#[description = "Unsets the channel for updates"]
#[only_in(guilds)]
fn unsetupdatechannel(ctx: &mut Context, msg: &Message) -> CommandResult {
	if let Some(guild_id) = msg.guild_id {
		let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
		let lock = canary.lock().unwrap();
		
		let res = lock.unset_update_channel(&guild_id);
		if res.is_ok() {
			msg.channel_id.say(&ctx, "Unset canary update channel").log_err();
		}
		else {
			msg.channel_id.say(&ctx, "Sql bad").log_err();
			res.log_err();
		}
	}
	else {
		msg.channel_id.say(&ctx, "Well how tf did this happen").log_err();
	}
	Ok(())
}

impl TypeMapKey for CanaryUpdateHandler {
	 type Value = Arc<Mutex<CanaryUpdateHandler>>;
}

pub struct CanaryUpdateHandler {
	possible_canary_updates: Arc<Mutex<Vec<CanaryUpdate>>>,
	sql_connection: Connection
}

impl CanaryUpdateHandler {
	pub fn new(framework: &mut StandardFramework) -> Self {
		framework.group_add(&CANARYUPDATECOMMANDS_GROUP);
		let possible_canary_updates: Arc<Mutex<Vec<CanaryUpdate>>> = Default::default();
		let sql_connection: Connection = Connection::open("oofbot.db").unwrap();
		Self {possible_canary_updates, sql_connection}
	}
    /// Spawns the canary update thread
	pub fn spawn_thread(&mut self) {
		let pcu = self.possible_canary_updates.clone();
		thread::spawn(move || {
			loop {
				let mut lock = pcu.lock().unwrap();
				let data: &mut Vec<CanaryUpdate> = &mut *lock;
				// Arbitrary capacity
				let mut drops: Vec<usize> = Vec::<usize>::with_capacity(data.len() / 2);
				for i in 0..data.len() {
					let t: Instant = data[i].time;
					if t.elapsed() > Duration::from_secs(20) {
						drops.push(i);
					}
				}
				for i in drops {
					data.remove(i);
				}
				drop(lock);
				std::thread::sleep(Duration::from_secs(2));
			}
		});
	}
    /// Adds a user to the list of canary updates
	pub fn add_canary_update(&mut self, user_id: &UserId) {
		if self.contains(user_id) {
			return;
		}
		let mut lock = self.possible_canary_updates.lock().unwrap();
		let data: &mut Vec<CanaryUpdate> = &mut *lock;
		data.push(CanaryUpdate {user_id: *user_id, time: Instant::now()});
	}
    /// Removes a user from the list of canary updates
	pub fn remove_canary_update(&mut self, user_id: &UserId) -> bool {
		let mut lock = self.possible_canary_updates.lock().unwrap();
		let data: &mut Vec<CanaryUpdate> = &mut *lock;
		for i in 0..data.len() {
			if data[i].user_id == *user_id {
				data.remove(i);
				return true;
			}
		}
		false
	}
    /// Checks if user is in the list of canary updates
	pub fn contains(&self, user_id: &UserId) -> bool {
		let lock = self.possible_canary_updates.lock().unwrap();
		let data: &Vec<CanaryUpdate> = &*lock;
		for i in 0..data.len() {
			if data[i].user_id == *user_id {
				return true;
			}
		}
		false
	}
    /// Creates the sqlite database
	pub fn create_db(&self) -> CommandResult {
		self.sql_connection.execute("CREATE TABLE canary (guild_id  UNSIGNED BIG INT UNIQUE NOT NULL, channel_id UNSIGNED BIG INT UNIQUE NOT NULL)")?;
		Ok(())
	}
    /// Sets the servers update channel
	pub fn set_update_channel(&self, guild_id: &GuildId, channel_id: &ChannelId) -> Result<()> {
		self.sql_connection.execute(format!("REPLACE INTO canary VALUES ({}, {})", guild_id, channel_id))?;
		Ok(())
	}
    /// Gets the servers update channel
	pub fn get_update_channel(&self, guild_id: &GuildId) -> Option<ChannelId> {
		let mut cursor = self.sql_connection.prepare(format!("SELECT channel_id FROM canary WHERE guild_id = {}", guild_id)).unwrap().cursor();
		if let Some(row) = cursor.next().unwrap() {
			// Cast the i64 to a u64 since we are actually storing a u64.
			return Some(ChannelId(u64::from_ne_bytes(row[0].as_integer().unwrap().to_ne_bytes())));
		}
		None
	}
    /// Unsets a guilds update channel
	pub fn unset_update_channel(&self, guild_id: &GuildId) -> CommandResult {
		self.sql_connection.execute(format!("DELETE FROM canary WHERE guild_id = {}", guild_id))?;
		Ok(())
	}
	
}
/// Sends out a message to all guilds a user is a part of that have canary update messages enabled
pub fn do_update(ctx: &Context, data: &PresenceUpdateEvent) {
	let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
	
	let mut lock = canary.lock().unwrap();
	if lock.remove_canary_update(&data.presence.user_id) {
		let clock = ctx.cache.read();
		let cref = clock.deref();
		
		for guild in cref.guilds.values() {
			let glock = guild.read();
			let guild: &Guild = glock.deref();
			if guild.members.get(&data.presence.user_id).is_some() {
				if let Some(x) = lock.deref().get_update_channel(&guild.id) {
                    // Recently discord has been segfaulting for me. So most of the time my canary
                    // updates are just segfaults
                    if data.presence.user_id == 453344368913547265 {
				    	x.say(&ctx, MessageBuilder::new().push("Possible segmentation fault detected for ").user(data.presence.user_id)).log_err();
                    }
                    else {
				    	x.say(&ctx, MessageBuilder::new().push("Possible canary update detected for ").user(data.presence.user_id)).log_err();
                    }
				}
			}
		}
	}
}
/// Simple struct containing info about a possible canary update
pub struct CanaryUpdate {
	user_id: UserId,
	time: Instant
}
