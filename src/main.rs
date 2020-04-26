extern crate serenity;

extern crate ctrlc;
#[macro_use]
pub mod logger;
pub mod dogebotno;
pub mod canary_update;
pub mod voice;

use serenity::model::channel::GuildChannel;
use std::{
	sync::Mutex,
	sync::{
		Arc,
		atomic::{
			AtomicBool
		}
	},
    collections::HashSet,
    ops::DerefMut
};
use serenity::{
	model::{channel::{Message}, gateway::{Ready}, id::{
		GuildId,
		ChannelId,
		UserId
	}, user::{OnlineStatus}, event::{
		PresenceUpdateEvent,
        ResumedEvent
	}},
	prelude::*,
	Client,
	framework::standard::{
		*,
		macros::{
			*
		}
	},
	utils::MessageBuilder,
    client::bridge::gateway::ShardManager
};
use lazy_static::*;
use rand::Rng;
use regex::Regex;
use canary_update::*;
use voice::OofVoice;




/// Unwrapping many of the errors in oofbot, mostly api calls, will result in a panic sometimes.
/// This is bad. But I also cant ignore the errors in case theres something bad in there. So my
/// solution is this trait, which logs the error. If I look in the logs and see something bad, then
/// I know to recheck everything
trait LogResult {
	/// If the result is an error, log the error.
	fn log_err(&self) where Self: std::fmt::Debug {
		log_timestamp!("DEBUG", format!("{:?}", self))
	}
}
impl<T: std::fmt::Debug, E: std::fmt::Debug> LogResult for Result<T, E> {
	/// If the result is an error, log the error.
	fn log_err(&self) {
		if self.is_err() {
			log_timestamp!("DEBUG", format!("{:?}", self))
		}
	}
}

/// The general command group. May be deleted later
#[group]
#[commands(test, executeorder66)]
struct General;

/// A testing command that can only be run by me.
#[command]
fn test(ctx: &mut Context, msg: &Message) -> CommandResult {
	if msg.author.id != 453344368913547265 {
		msg.channel_id.say(&ctx, "No").log_err();
		return Ok(())
	}
	let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
	let lock = canary.lock()?;
	let res = lock.create_db();
	res.log_err();
	if res.is_ok() { msg.channel_id.say(&ctx, "It seems to have worked").log_err();
	}
	else {
		msg.channel_id.say(&ctx, "killme").log_err();
	}
	Ok(())
}
#[command]
fn executeorder66(ctx: &mut Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx, "not yet").log_err();
    Ok(())
}

/// The event handler for oofbot
pub struct Handler {
	cancel_tyler_ping: Arc<AtomicBool>,
    mention_regex: Regex
}

impl Default for Handler {
    fn default() -> Self {
        Self {cancel_tyler_ping: Arc::default(), mention_regex: Regex::new(r"<@!?468928390917783553>").unwrap()}
	}
}

impl EventHandler for Handler {
	fn presence_update(&self, ctx: Context, data: PresenceUpdateEvent) {
        // oofbot only handles guild presence updates
		if data.guild_id.is_none() {
			return;
		}
        // Dogebot is oofbots greatest enemy. We got some checks in here just for him.
        let is_dogebot = data.presence.user_id == 612070962913083405;
        // Should never be none because we check that up there
		let guild_id = data.guild_id.unwrap();
        // Checks if dogebot is offline in this guild (the main development guild for dogebot and
        // oofbot)
	    if is_dogebot && guild_id.0 == 561874457283657728 {
			dogebotno::dogebot_presence(&ctx, &data, &guild_id, self);
		}
        else if !is_dogebot && data.presence.status == OnlineStatus::Offline {
            // Inside joke, memeing on how tiny discord canary updates are and how often we get them
			let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
			let mut lock = canary.lock().unwrap();
			lock.add_canary_update(&data.presence.user_id);
		}
		else if !is_dogebot && data.presence.status == OnlineStatus::Online {
		    canary_update::do_update(&ctx, &data);
		}
	}
	fn resume(&self, _ctx: Context, _data: ResumedEvent) {
		log_timestamp!("INFO", "Reconnected to discord");
	}
	fn ready(&self, ctx: Context, _data: Ready) {
        log_timestamp!("INFO", format!("Shard {} ready", ctx.shard_id));
        
	}
    fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        let shard = ctx.shard_id;
        // Get all the guilds that this shard is connected to
        // Not that this bot will ever be big enough for me to bother sharding it
        let guild_info: Vec<(&GuildId, String)> = guilds.iter().filter_map(|guild_id| {
            if guild_id.shard_id(&ctx) == ctx.shard_id {
                return Some((guild_id, guild_id.to_guild_cached(&ctx).unwrap().read().name.clone()));
            }
            else {
                return None;
            }
        }).collect();
    
        log_timestamp!("INFO", format!("Shard {} connected to guilds\n{:#?}", shard, guild_info));
    }
	fn message(&self, ctx: Context, msg: Message) {
		if msg.author.id == 612070962913083405 {
			dogebotno::dogebotno(ctx, msg);
			return;
		}
        if self.mention_regex.is_match(msg.content.as_str()) {
			let channel_id: ChannelId = msg.channel_id;
            channel_id.say(&ctx, "For thousands of years I lay dormant, who has disturbed my slumber").log_err();
            return;
        }
		if msg.content.contains("@someone") && !msg.author.bot {
			return someone_ping(ctx, msg);
		}
		if (msg.content.contains("@everyone") || msg.content.contains("@here")) && msg.author.id.0 != 468928390917783553 {
			msg.channel_id.say(&ctx, "https://kikoho.ddns.net/files/ping.gif").log_err();
		}
		
	}
}

fn main() {
	log_timestamp!("INFO", "Starting oofbot");
    log_timestamp!("INFO", "Getting client secret from file");

    let secret = std::fs::read_to_string("client_secret").expect("Client secret needs to be in a file called client_secret");
	let mut client: Client = Client::new(secret, Handler::default()).expect("Client Creation Failed");
	let mut framework = StandardFramework::new()
			.configure(|c| c.prefix("/"))
			.group(&GENERAL_GROUP)
            .group(&ADMIN_GROUP)
			.help(&HELP);

	// Voice initialization
	{
		// Lock the clients data
		let mut data = client.data.write();
		// Add the voice manager
        log_timestamp!("INFO", "Starting oofvoice");
		data.insert::<OofVoice>(OofVoice::new(client.voice_manager.clone(), &mut framework));
        log_timestamp!("INFO", "Started oofvoice");
		// Add canary update handler
        log_timestamp!("INFO", "Starting canary update handler");
		data.insert::<CanaryUpdateHandler>(Arc::new(Mutex::new(CanaryUpdateHandler::new(&mut framework))));
	    log_timestamp!("INFO", "Started canary update handler");
	}
	client.with_framework(framework);
	let shard_manager = client.shard_manager.clone();
    // Handle ctrl+c cross platform
	ctrlc::set_handler(move || {
		log_timestamp!("INFO", "Caught SIGINT, closing oofbot");
		let mut lock = shard_manager.lock();
		let sm: &mut ShardManager = lock.deref_mut();
		sm.shutdown_all();
		std::process::exit(0);
	}).log_err();

    // Hah you think this bot is big enough to be sharded? Nice joke
	client.start().expect("oof");
}
/// Handles the @someone ping. Yes im evil.
fn someone_ping(ctx: Context, msg: Message) {
	let guild_id: Option<GuildId> = msg.guild_id;
	let channel_id: ChannelId = msg.channel_id;
	match guild_id {
		Some(guild) => {
			let mut message = MessageBuilder::new();
			{
				let clock = ctx.cache.read();
				let cref = clock.deref();
				let asdf = &cref.guilds;
				let glock = &asdf[&guild].deref().read();
				let mem: Vec<&UserId> = glock.members.keys().into_iter().collect();
				

				let mut rng = rand::thread_rng();
				// Add from message
				message.push("From ").user(&msg.author);
				message.quote_rest();
				
				// Randomly select the @someones
				let m: Vec<&str> = msg.content.split("@someone").collect();
				for i in 0..m.len()-1 {
					message.push(m[i]).user(mem[rng.gen_range(0, mem.len())]);
				}
				message.push(m.last().unwrap());
			}
			channel_id.say(&ctx, message).log_err();
			msg.delete(&ctx).log_err();
		}
		None => {
			// If guild is none then this is a dm
			channel_id.say(&ctx.http, "Cannot @someone in dms").log_err();
			()
		}
	}
	
}

#[help]
fn help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners)
}

#[check]
#[name = "ManageMessages"]
#[check_in_help(true)]
#[display_in_help(true)]
fn manage_messages_check(ctx: &mut Context, msg: &Message) -> CheckResult {
    if msg.author.id == 453344368913547265 {
        return true.into()
    } 
    else if let Some(member) = msg.member(&ctx.cache) {
        if let Ok(permissions) = member.permissions(&ctx.cache) {
            return (permissions.administrator() || permissions.manage_messages()).into();
        }
    }

    false.into()
}

#[check]
#[name = "Manage"]
#[check_in_help(true)]
#[display_in_help(true)]
fn manage_check(ctx: &mut Context, msg: &Message) -> CheckResult {
    if msg.author.id == 453344368913547265 {
        return true.into()
    } 
    else if let Some(member) = msg.member(&ctx.cache) {
        if let Ok(permissions) = member.permissions(&ctx.cache) {
            return (permissions.administrator() || permissions.manage_guild()).into();
        }
    }

    false.into()
}
#[group]
#[commands(snapbm)]
/// Admin command group
/// Get this, it has admin commands, amazing right?
struct Admin;

#[command]
#[checks(ManageMessages)]
#[description = "Scan the last X messages and delete all that are from bots. Messages older than 2 weeks cannot be deleted with this command. Maximum of 300 messages"]
#[only_in(guilds)]
/// Scan the last X messages and delete all that are from bots. Messages older than 2 weeks cannot be deleted with this command. Maximum of 300 messages
fn snapbm(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let count = match args.single::<u64>() {
        Ok(x) if x <= 300 => x,
        _ => {
            msg.channel_id.say(&ctx, "Usage: /snapbm <number>").log_err();
            return Ok(());
        }
    };
    let l_channel: Arc<RwLock<GuildChannel>> = msg.channel_id.to_channel(&ctx).unwrap().guild().unwrap();
    let channel = l_channel.read();
    let messages = channel.messages(&ctx, |retriever| {
        retriever.before(msg.id).limit(count)
    })?;
    let bot_messages: Vec<&Message> = messages.iter().filter(|msg| {msg.author.bot}).collect();
    channel.delete_messages(&ctx, bot_messages).log_err();
    Ok(())
}
