#![deny(unused_must_use)]
#![type_length_limit = "1340885"]
extern crate serenity;

extern crate ctrlc;
#[macro_use]
pub mod logger;
pub mod canary_update;
pub mod dogebotno;
pub mod permissions;
pub mod servers;
pub mod voice;
use canary_update::*;
use futures::{Stream, StreamExt};
use lazy_static::*;
use logger::get_guild_members;
use rand::Rng;
use regex::Regex;
use serenity::async_trait;
use serenity::client::bridge::gateway::GatewayIntents;
use serenity::model::channel::GuildChannel;
use serenity::{
	client::bridge::gateway::ShardManager,
	framework::standard::{macros::*, *},
	model::{
		channel::Message,
		event::{PresenceUpdateEvent, ResumedEvent},
		gateway::Ready,
		id::{ChannelId, GuildId, UserId},
		user::OnlineStatus,
	},
	prelude::*,
	utils::MessageBuilder,
	Client,
};
use std::{
	collections::HashSet,
	ops::DerefMut,
	sync::{atomic::AtomicBool, Arc},
};
use tokio::{stream, sync::Mutex};
use voice::OofVoice;

/// Unwrapping many of the errors in oofbot, mostly api calls, will result in a panic sometimes.
/// This is bad. But I also cant ignore the errors in case theres something bad in there. So my
/// solution is this trait, which logs the error. If I look in the logs and see something bad, then
/// I know to recheck everything
trait LogResult {
	/// If the result is an error, log the error.
	fn log_err(&self)
	where
		Self: std::fmt::Debug,
	{
		log_timestamp!("DEBUG", format!("{:?}", self))
	}
}
impl<T: std::fmt::Debug, E: std::fmt::Debug> LogResult for Result<T, E> {
	/// If the result is an error, log the error.
	fn log_err(&self) {
		if self.is_err() {
			log_timestamp!("DEBUG", format!("{:?}", self));
		}
	}
}

/// The general command group. May be deleted later
#[group]
#[commands(test, executeorder66, getdvsstatus)]
struct General;
/// A testing command that can only be run by me.
#[command]
async fn test(ctx: &Context, msg: &Message) -> CommandResult {
	if msg.author.id != 453344368913547265 {
		msg.channel_id.say(&ctx, "No").await.log_err();
		return Ok(());
	}
	//let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
	//let lock = canary.lock()?;
	//let res = lock.create_db();
	//res.log_err();
	//if res.is_ok() { msg.channel_id.say(&ctx, "It seems to have worked").log_err();
	//}
	//else {
	//	msg.channel_id.say(&ctx, "killme").log_err();
	//}
	msg.channel_id.say(&ctx, "@admin").await.log_err();
	Ok(())
}
#[command]
async fn executeorder66(ctx: &Context, msg: &Message) -> CommandResult {
	msg.channel_id.say(&ctx, "not yet").await.log_err();
	Ok(())
}

/// The event handler for oofbot
pub struct Handler {
	cancel_tyler_ping: Arc<AtomicBool>,
	mention_regex: Regex,
}

impl Default for Handler {
	fn default() -> Self {
		Self {
			cancel_tyler_ping: Arc::default(),
			mention_regex: Regex::new(r"<@!?468928390917783553>").unwrap(),
		}
	}
}

#[async_trait]
impl EventHandler for Handler {
	async fn presence_update(&self, ctx: Context, data: PresenceUpdateEvent) {
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
			dogebotno::dogebot_presence(&ctx, &data, &guild_id, self).await;
		} else if !is_dogebot && data.presence.status == OnlineStatus::Offline {
			// Inside joke, memeing on how tiny discord canary updates are and how often we get them
			let canary = ctx
				.data
				.read()
				.await
				.get::<CanaryUpdateHandler>()
				.cloned()
				.unwrap();
			let mut lock = canary.lock().await;
			lock.add_canary_update(&data.presence.user_id).await;
		} else if !is_dogebot && data.presence.status == OnlineStatus::Online {
			canary_update::do_update(&ctx, &data).await;
		}
	}
	async fn resume(&self, _ctx: Context, _data: ResumedEvent) {
		log_timestamp!("INFO", "Reconnected to discord");
	}
	async fn ready(&self, ctx: Context, _data: Ready) {
		log_timestamp!("INFO", format!("Shard {} ready", ctx.shard_id));
	}
	async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
		let shard = ctx.shard_id;
		let rctx = &ctx;
		// Get all the guilds that this shard is connected to
		// Not that this bot will ever be big enough for me to bother sharding it
		let guild_info: Vec<_> = stream::iter(guilds)
			.filter_map(|guild_id| async move {
				if guild_id.shard_id(&rctx).await == rctx.shard_id {
					Some((
						guild_id,
						guild_id.to_guild_cached(&rctx).await.unwrap().name.clone(),
					))
				} else {
					None
				}
			})
			.collect()
			.await;
		log_timestamp!(
			"INFO",
			format!("Shard {} connected to guilds\n{:#?}", shard, guild_info)
		);
	}
	async fn message(&self, ctx: Context, msg: Message) {
		log_timestamp!("DEBUG", &msg.content);
		if msg.author.id == 612070962913083405 {
			dogebotno::dogebotno(ctx, msg).await;
			return;
		}
		if self.mention_regex.is_match(msg.content.as_str()) {
			let channel_id: ChannelId = msg.channel_id;
			channel_id
				.say(
					&ctx,
					"For thousands of years I lay dormant, who has disturbed my slumber",
				)
				.await
				.log_err();
		}
		if msg.content.contains("@someone") && !msg.author.bot {
			someone_ping(&ctx, &msg).await;
		}
		if (msg.content.contains("@everyone") || msg.content.contains("@here"))
			&& msg.author.id.0 != 468928390917783553
		{
			msg.channel_id
				.say(&ctx, "https://yeet.kikoho.xyz/files/ping.gif")
				.await
				.log_err();
		}
		if msg.author.id == 266345279513427971
			&& msg.content.contains("https://www.twitch.tv/corporal_q")
		{
			msg.channel_id.say(&ctx, "sotp spamming").await.log_err();
		}
	}
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	log_timestamp!("INFO", "Starting oofbot");
	log_timestamp!("INFO", "Getting client secret from file");

	let mut framework = StandardFramework::new()
		.configure(|c| c.prefix("/"))
		.group(&GENERAL_GROUP)
		.group(&ADMIN_GROUP)
		.help(&HELP);
	voice::do_framework(&mut framework);
	permissions::do_framework(&mut framework);
	canary_update::do_framework(&mut framework);

	let secret = std::fs::read_to_string("client_secret")
		.expect("Client secret needs to be in a file called client_secret");
	let mut client = Client::builder(secret)
		.add_intent(GatewayIntents::all())
		.framework(framework)
		.event_handler(Handler::default())
		.await
		.expect("Failed to create client");

	// Voice initialization
	{
		// Lock the clients data
		let mut data = client.data.write().await;
		// Add the voice manager
		log_timestamp!("INFO", "Starting oofvoice");
		data.insert::<OofVoice>(OofVoice::new(client.voice_manager.clone()).await);
		log_timestamp!("INFO", "Started oofvoice");
		// Add canary update handler
		log_timestamp!("INFO", "Starting canary update handler");
		let sql = permissions::SqlHandler::new();
		data.insert::<CanaryUpdateHandler>(Arc::new(Mutex::new(CanaryUpdateHandler::new(sql))));
		log_timestamp!("INFO", "Started canary update handler");
	}
	let shard_manager = client.shard_manager.clone();
	// Handle ctrl+c cross platform
	ctrlc::set_handler(move || {
		log_timestamp!("INFO", "Caught SIGINT, closing oofbot");
		//let mut lock = shard_manager.lock().await;
		//let sm: &mut ShardManager = lock.deref_mut();
		//sm.shutdown_all();
		std::process::exit(0);
	})
	.log_err();

	// Hah you think this bot is big enough to be sharded? Nice joke
	// But if yours is use .start_autosharded()
	client.start().await?;
	Ok(())
}
/// Handles the @someone ping. Yes im evil.
async fn someone_ping(ctx: &Context, msg: &Message) {
	let guild_id: Option<GuildId> = msg.guild_id;
	let channel_id: ChannelId = msg.channel_id;
	match guild_id {
		Some(id) => {
			let mut message = MessageBuilder::new();
			{
				let members = match get_guild_members(&ctx, id).await {
					Some(m) => m,
					None => {
						log_timestamp!("ERROR", format!("Failed to find guild {}", id));
						msg.channel_id.say(&ctx, "Internal Error").await.log_err();
						return;
					}
				};

				let mut rng = rand::thread_rng();

				message.mention(&msg.author);
				message.push(" has pinged: ");

				let someones = msg.content.split("@someone");
				let c = someones.count();
				if c > 1 {
					let r = rng.gen_range(0, members.len());
					message.mention(&members[r]);
				}

				// Randomly select the @someones
				msg.content.split("@someone").skip(2).for_each(|_| {
					message.push(", ");
					let r = rng.gen_range(0, members.len());
					message.mention(&members[r]);
				});
			}
			channel_id.say(&ctx, message).await.log_err();
		}
		None => {
			// If guild is none then this is a dm
			channel_id
				.say(&ctx.http, "Cannot @someone in dms")
				.await
				.log_err();
		}
	}
}

#[help]
async fn help(
	context: &Context,
	msg: &Message,
	args: Args,
	help_options: &'static HelpOptions,
	groups: &[&'static CommandGroup],
	owners: HashSet<UserId>,
) -> CommandResult {
	help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
	Ok(())
}

#[check]
#[name = "ManageMessages"]
#[check_in_help(true)]
#[display_in_help(true)]
async fn manage_messages_check(ctx: &Context, msg: &Message) -> CheckResult {
	if msg.author.id == 453344368913547265 {
		return true.into();
	} else if let Ok(member) = msg.member(&ctx).await {
		if let Ok(permissions) = member.permissions(&ctx.cache).await {
			return (permissions.administrator() || permissions.manage_messages()).into();
		}
	}

	false.into()
}
#[check]
#[name = "DVS"]
#[check_in_help(true)]
#[display_in_help(true)]
async fn dvs_check(_ctx: &Context, msg: &Message) -> CheckResult {
	(msg.guild_id.unwrap_or(0.into()) == 693213312099287153).into()
}

#[group]
#[commands(snapbm, snapping, snapbotcommands, snapspam, snapafter, setslowmode)]
/// Admin command group
/// Get this, it has admin commands, amazing right?
struct Admin;

#[command]
#[checks(ManageMessages)]
#[only_in(guilds)]
/// Scan the last X messages and delete all that are from bots. Messages older than 2 weeks cannot be deleted with this command. Maximum of 500 messages
async fn snapbm(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let count = match args.single::<u64>() {
		Ok(x) if x <= 500 => x,
		_ => {
			msg.channel_id
				.say(&ctx, "Usage: /snapbm <number>")
				.await
				.log_err();
			return Ok(());
		}
	};
	let channel: GuildChannel = msg
		.channel_id
		.to_channel(&ctx)
		.await
		.unwrap()
		.guild()
		.unwrap();
	let messages = channel
		.messages(&ctx, |retriever| retriever.before(msg.id).limit(count))
		.await?;
	let mut bot_messages: Vec<&Message> = messages
		.iter()
		.filter(|msg| {
			msg.author.bot
				&& chrono::Utc::now().naive_utc() - msg.timestamp.naive_utc()
					< chrono::Duration::weeks(2)
		})
		.collect();
	bot_messages.push(msg);
	channel.delete_messages(&ctx, bot_messages).await.log_err();
	Ok(())
}
#[command]
#[checks(ManageMessages)]
#[only_in(guilds)]
/// Scan the last X messages and delete all that contain pings. Messages older than 2 weeks cannot be deleted with this command. Maximum of 500 messages
async fn snapping(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let ping_regex = Regex::new("<@!?\\d*>").unwrap();
	let count = match args.single::<u64>() {
		Ok(x) if x <= 500 => x,
		_ => {
			msg.channel_id
				.say(&ctx, "Usage: /lazysnapping <number>")
				.await
				.log_err();
			return Ok(());
		}
	};
	let channel = msg
		.channel_id
		.to_channel(&ctx)
		.await
		.unwrap()
		.guild()
		.unwrap();
	let messages = channel
		.messages(&ctx, |retriever| retriever.before(msg.id).limit(count))
		.await?;
	let mut bot_messages: Vec<&Message> = messages
		.iter()
		.filter(|msg| {
			ping_regex.is_match(msg.content.as_str())
				&& chrono::Utc::now().naive_utc() - msg.timestamp.naive_utc()
					< chrono::Duration::weeks(2)
		})
		.collect();
	bot_messages.push(msg);
	channel.delete_messages(&ctx, bot_messages).await.log_err();
	Ok(())
}

#[command]
#[checks(ManageMessages)]
#[only_in(guilds)]
/// Scan the last X messages and delete all that start with /. Messages older than 2 weeks cannot be deleted with this command. Maximum of 500 messages
async fn snapbotcommands(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let count = match args.single::<u64>() {
		Ok(x) if x <= 500 => x,
		_ => {
			msg.channel_id
				.say(&ctx, "Usage: /snapbotcommands <number>")
				.await
				.log_err();
			return Ok(());
		}
	};
	let channel = msg
		.channel_id
		.to_channel(&ctx)
		.await
		.unwrap()
		.guild()
		.unwrap();
	let messages = channel
		.messages(&ctx, |retriever| retriever.before(msg.id).limit(count))
		.await?;
	let mut bot_messages: Vec<&Message> = messages
		.iter()
		.filter(|msg| {
			(msg.content.starts_with('/') || msg.content.starts_with('!'))
				&& chrono::Utc::now().naive_utc() - msg.timestamp.naive_utc()
					< chrono::Duration::weeks(2)
		})
		.collect();
	bot_messages.push(msg);
	channel.delete_messages(&ctx, bot_messages).await.log_err();
	Ok(())
}
#[command]
#[checks(ManageMessages)]
#[only_in(guilds)]
/// Murder the last X messages. Messages older than 2 weeks cannot be deleted with this command. Maximum of 500 messages
async fn snapspam(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let count = match args.single::<u64>() {
		Ok(x) if x <= 500 => x,
		_ => {
			msg.channel_id
				.say(&ctx, "Usage: /snapspam <number>")
				.await
				.log_err();
			return Ok(());
		}
	};
	let channel = msg
		.channel_id
		.to_channel(&ctx)
		.await
		.unwrap()
		.guild()
		.unwrap();
	let mut messages = channel
		.messages(&ctx, |retriever| retriever.before(msg.id).limit(count))
		.await?;
	messages.push(msg.clone());
	let messages = messages.into_iter().filter(|m| {
		chrono::Utc::now().naive_utc() - m.timestamp.naive_utc() < chrono::Duration::weeks(2)
	});
	channel.delete_messages(&ctx, messages).await.log_err();
	Ok(())
}

#[command]
#[only_in(guilds)]
#[checks(DVS)]
/// Gets the status of the DVS minecraft server
async fn getdvsstatus(ctx: &Context, msg: &Message) -> CommandResult {
	msg.channel_id.broadcast_typing(&ctx).await.log_err();
	let code = std::process::Command::new("sh")
		.args(&[
			"-c",
			"nmap -4 applesthepi.com -Pn -p 25566 | rg '25566/tcp open'",
		])
		.status()
		.unwrap();
	if code.success() {
		let message = MessageBuilder::new()
			.user(msg.author.id)
			.push(" Server port appears to be open, so it should be up.")
			.build();
		msg.channel_id.say(&ctx, message).await.log_err();
	} else {
		msg.channel_id
			.say(
				&ctx,
				"Server down indeed, <@324381278600298509> your server is on crack",
			)
			.await
			.log_err();
	}
	Ok(())
}

#[command]
#[checks(ManageMessages)]
#[only_in(guilds)]
/// Murder all messages after the message with the given id. Message ids can be gotten by enabling
/// developer mode in discord setting and right click -> copy id
/// Messages older than 2 weeks cannot be deleted with this.
/// Usage: /snapuntil messageid
async fn snapafter(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let id = args.single::<u64>()?;

	let channel = msg
		.channel_id
		.to_channel(&ctx)
		.await
		.unwrap()
		.guild()
		.unwrap();

	let messages = channel
		.messages(&ctx, |retriever| retriever.after(id))
		.await?;
	let messages = messages.into_iter().filter(|m| {
		chrono::Utc::now().naive_utc() - m.timestamp.naive_utc() < chrono::Duration::weeks(2)
	});
	channel.delete_messages(&ctx, messages).await.log_err();
	Ok(())
}

#[command]
#[checks(ManageMessages)]
#[only_in(guilds)]
/// Sets the slowmode to any second value. This allow more specific slow mode like 1 second.
/// Usage: /setslowmode integer
async fn setslowmode(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let arg: u64 = args.single()?;
	msg.channel_id
		.to_channel(&ctx)
		.await?
		.guild()
		.unwrap()
		.edit(&ctx, |c| c.slow_mode_rate(arg))
		.await
		.log_err();
	Ok(())
}
