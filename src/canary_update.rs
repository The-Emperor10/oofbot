use crate::logger::get_guild_members;
use crate::{permissions::SqlHandler, LogResult};
use serenity::framework::standard::macros::check;
use serenity::{
	framework::standard::{
		macros::{command, group},
		*,
	},
	model::prelude::*,
	prelude::*,
	utils::MessageBuilder,
};
use sqlite::*;
use std::{
	ops::Deref,
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::sync::Mutex;
pub fn do_framework(framework: &mut StandardFramework) {
	framework.group_add(&CANARYUPDATECOMMANDS_GROUP);
}
#[check]
#[name = "Manage"]
#[check_in_help(true)]
#[display_in_help(true)]
async fn manage_check(ctx: &Context, msg: &Message) -> CheckResult {
	if msg.author.id == 453344368913547265 {
		return true.into();
	} else if let Ok(member) = msg.member(&ctx).await {
		if let Ok(permissions) = member.permissions(&ctx.cache).await {
			return (permissions.administrator() || permissions.manage_guild()).into();
		}
	}

	false.into()
}
#[group]
#[commands(setupdatechannel, getupdatechannel, unsetupdatechannel)]
#[description = "Commands related to canary updates"]
struct CanaryUpdateCommands;

#[command]
#[checks(Manage)]
#[description = "Sets the channel for updates"]
#[only_in(guilds)]
async fn setupdatechannel(ctx: &Context, msg: &Message) -> CommandResult {
	if let Some(guild_id) = msg.guild_id {
		let clock = ctx.data.read().await;
		let canary = clock.get::<CanaryUpdateHandler>().unwrap();
		let lock = canary.lock().await;

		let res = lock.set_update_channel(&guild_id, &msg.channel_id).await;
		if res.is_ok() {
			msg.channel_id
				.say(
					&ctx,
					"Successfully set this channel to the canary update notification channel",
				)
				.await
				.log_err();
		} else {
			msg.channel_id.say(&ctx, "Sql bad").await.log_err();
			res.log_err();
		}
	} else {
		msg.channel_id
			.say(&ctx, "Well how tf did this happen")
			.await
			.log_err();
	}
	Ok(())
}
#[command]
#[description = "Gets the channel for updates"]
#[only_in(guilds)]
async fn getupdatechannel(ctx: &Context, msg: &Message) -> CommandResult {
	if let Some(guild_id) = msg.guild_id {
		let clock = ctx.data.read().await;
		let canary = clock.get::<CanaryUpdateHandler>().unwrap();
		let lock = canary.lock().await;

		let res = lock.get_update_channel(&guild_id).await;
		if let Some(id) = res {
			msg.channel_id
				.say(&ctx, MessageBuilder::new().channel(id))
				.await
				.log_err();
		} else {
			msg.channel_id.say(&ctx, "None").await.log_err();
		}
	} else {
		msg.channel_id
			.say(&ctx, "Well how tf did this happen")
			.await
			.log_err();
	}
	Ok(())
}

#[command]
#[checks(Manage)]
#[description = "Unsets the channel for updates"]
#[only_in(guilds)]
async fn unsetupdatechannel(ctx: &Context, msg: &Message) -> CommandResult {
	if let Some(guild_id) = msg.guild_id {
		let clock = ctx.data.read().await;
		let canary = clock.get::<CanaryUpdateHandler>().unwrap();
		let lock = canary.lock().await;

		let res = lock.unset_update_channel(&guild_id).await;
		if res.is_ok() {
			msg.channel_id
				.say(&ctx, "Unset canary update channel")
				.await
				.log_err();
		} else {
			msg.channel_id.say(&ctx, "Sql bad").await.log_err();
			res.log_err();
		}
	} else {
		msg.channel_id
			.say(&ctx, "Well how tf did this happen")
			.await
			.log_err();
	}
	Ok(())
}

impl TypeMapKey for CanaryUpdateHandler {
	type Value = Arc<Mutex<CanaryUpdateHandler>>;
}

pub struct CanaryUpdateHandler {
	possible_canary_updates: Arc<Mutex<Vec<CanaryUpdate>>>,
	sql_handler: Arc<SqlHandler>,
}

impl CanaryUpdateHandler {
	pub fn new(sql_handler: Arc<SqlHandler>) -> Self {
		let possible_canary_updates: Arc<Mutex<Vec<CanaryUpdate>>> = Default::default();
		Self {
			possible_canary_updates,
			sql_handler,
		}
	}
	/// Spawns the canary update thread
	pub async fn spawn_thread(&mut self) {
		let pcu = self.possible_canary_updates.clone();
		tokio::spawn(async move {
			loop {
				let mut lock = pcu.lock().await;
				let data: &mut Vec<CanaryUpdate> = &mut *lock;
				// Arbitrary capacity
				let mut drops: Vec<usize> = Vec::<usize>::with_capacity(data.len() / 2);
				for (i, update) in data.iter().enumerate() {
					let t: Instant = update.time;
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
	pub async fn add_canary_update(&mut self, user_id: &UserId) {
		if self.contains(user_id).await {
			return;
		}
		let mut lock = self.possible_canary_updates.lock().await;
		let data: &mut Vec<CanaryUpdate> = &mut *lock;
		data.push(CanaryUpdate {
			user_id: *user_id,
			time: Instant::now(),
		});
	}
	/// Removes a user from the list of canary updates
	pub async fn remove_canary_update(&mut self, user_id: &UserId) -> bool {
		let mut lock = self.possible_canary_updates.lock().await;
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
	pub async fn contains(&self, user_id: &UserId) -> bool {
		let lock = self.possible_canary_updates.lock().await;
		let data: &Vec<CanaryUpdate> = &*lock;
		for update in data {
			if update.user_id == *user_id {
				return true;
			}
		}
		false
	}
	/// Sets the servers update channel
	pub async fn set_update_channel(
		&self,
		guild_id: &GuildId,
		channel_id: &ChannelId,
	) -> Result<()> {
		self.sql_handler
			.sql_connection
			.lock()
			.await
			.execute(format!(
				"REPLACE INTO canary VALUES ({}, {})",
				guild_id, channel_id
			))?;
		Ok(())
	}
	/// Gets the servers update channel
	pub async fn get_update_channel(&self, guild_id: &GuildId) -> Option<ChannelId> {
		let sql = self.sql_handler.sql_connection.lock().await;
		let mut cursor = sql
			.prepare(format!(
				"SELECT channel_id FROM canary WHERE guild_id = {}",
				guild_id
			))
			.unwrap()
			.cursor();
		if let Some(row) = cursor.next().unwrap() {
			// Cast the i64 to a u64 since we are actually storing a u64.
			return Some(ChannelId(u64::from_ne_bytes(
				row[0].as_integer().unwrap().to_ne_bytes(),
			)));
		}
		None
	}
	/// Unsets a guilds update channel
	pub async fn unset_update_channel(&self, guild_id: &GuildId) -> CommandResult {
		self.sql_handler
			.sql_connection
			.lock()
			.await
			.execute(format!("DELETE FROM canary WHERE guild_id = {}", guild_id))?;
		Ok(())
	}
}
/// Sends out a message to all guilds a user is a part of that have canary update messages enabled
pub async fn do_update(ctx: &Context, data: &PresenceUpdateEvent) {
	let lk = ctx.data.read().await;
	let canary = lk.get::<CanaryUpdateHandler>().unwrap();
	let id = data.guild_id.unwrap();
	let mut lock = canary.lock().await;
	if lock.remove_canary_update(&data.presence.user_id).await {
		for guild in ctx.cache.guilds().await {
			let members;
			if let Some(guild) = id.to_guild_cached(&ctx).await {
				members = guild.members(&ctx, None, None).await.unwrap();
			} else if let Ok(guild) = id.to_partial_guild(&ctx).await {
				members = guild.members(&ctx, None, None).await.unwrap();
			} else {
				log_timestamp!("ERROR", format!("Failed to find guild {}", id));
				return;
			}

			let members = match get_guild_members(&ctx, id).await {
				Some(m) => m,
				None => {
					log_timestamp!("ERROR", format!("Failed to find guild {}", id));
					return;
				}
			};
			if members
				.iter()
				.find(|m| m.user.id == data.presence.user_id)
				.is_some()
			{
				if let Some(x) = lock.deref().get_update_channel(&id).await {
					// Recently discord has been segfaulting for me. So most of the time my canary
					// updates are just segfaults
					if data.presence.user_id == 453344368913547265 {
						x.say(
							&ctx,
							MessageBuilder::new()
								.push("Possible segmentation fault detected for ")
								.user(data.presence.user_id),
						)
						.await
						.log_err();
					} else {
						x.say(
							&ctx,
							MessageBuilder::new()
								.push("Possible canary update detected for ")
								.user(data.presence.user_id),
						)
						.await
						.log_err();
					}
				}
			}
		}
	}
}
/// Simple struct containing info about a possible canary update
pub struct CanaryUpdate {
	user_id: UserId,
	time: Instant,
}
