use crate::LogResult;
use serenity::model::id::GuildId;
use serenity::{
	client::bridge::voice::*,
	framework::standard::{
		macros::{command, group},
		*,
	},
	model::channel::Message,
	prelude::*,
	utils::MessageBuilder,
	voice::*,
};
use std::collections::HashMap;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::{
	ffi::OsString,
	fs,
	ops::{Deref, DerefMut},
	sync::Arc,
	thread,
	time::Duration,
};

impl TypeMapKey for OofVoice {
	type Value = Arc<RwLock<OofVoice>>;
}
#[group]
#[commands(join, leave, play, stop, queue, remove, list)]
struct Voice;
use serenity::prelude::Mutex as SerMutex;
/// Yes all those RwLocks are necessary
pub struct OofVoice {
	pub voice_manager: Arc<SerMutex<ClientVoiceManager>>,
	pub sources: Arc<RwLock<HashMap<GuildId, (LockedAudio, OsString)>>>,
	// TODO: Replace with custom queue type
	pub queue: Arc<RwLock<HashMap<GuildId, RwLock<Vec<OsString>>>>>,
}

pub fn do_framework(framework: &mut StandardFramework) {
	framework.group_add(&VOICE_GROUP);
}

impl OofVoice {
	pub async fn new(voice_manager: Arc<SerMutex<ClientVoiceManager>>) -> Arc<RwLock<Self>> {
		let s = Arc::new(RwLock::new(Self {
			voice_manager,
			sources: Default::default(),
			queue: Default::default(),
		}));
		let oofvoice = s.clone();

		// Spaghetti warning
		tokio::spawn(async move {
			let mut timer = tokio::time::interval(Duration::from_secs(5));
			loop {
				timer.tick().await;
				let oof = oofvoice.read().await;
				let sources = oof.sources.read().await;
				let mut next: Vec<(GuildId, Option<OsString>)> =
					Vec::with_capacity(sources.deref().len());
				for i in sources.deref() {
					if (i.1).0.lock().await.finished {
						next.push((*i.0, None));
					}
				}
				drop(sources);
				let queue = oof.queue.read().await;

				for mut i in &mut next {
					if let Some(vec) = queue.get(&i.0) {
						let mut vec = vec.write().await;
						if vec.len() > 0 {
							i.1 = Some(vec.remove(0));
						}
					}
				}
				drop(queue);
				for i in next {
					if let Some(queued) = i.1 {
						let handler;
						let mut lock = oof.voice_manager.lock().await;
						let voice: &mut ClientVoiceManager = lock.deref_mut();
						handler = match voice.get_mut(i.0) {
							Some(x) => x,
							None => continue,
						};
						oof.play_file(queued, handler, i.0).await.log_err();
					}
				}
			}
		});
		s
	}
	pub async fn join(&self, ctx: &Context, msg: &Message) -> bool {
		let guild_id = msg.guild_id.unwrap();
		let mut lock = self.voice_manager.lock().await;
		let voice: &mut ClientVoiceManager = lock.deref_mut();

		let guild = msg.guild(&ctx).await.unwrap();

		let channel_id = guild
			.voice_states
			.get(&msg.author.id)
			.and_then(|voice_state| voice_state.channel_id);
		if let Some(channel) = channel_id {
			if voice.join(guild_id, channel).is_some() {
				log_timestamp!(
					"VOICE/INFO",
					format!("Joined voice channel {} in guild {}", channel, guild_id)
				);
				true
			} else {
				msg.channel_id
					.say(&ctx, "Failed to join the channel")
					.await
					.log_err();
				log_timestamp!(
					"VOICE/ERROR",
					format!(
						"Failed to join voice channel {} in guild {}",
						channel, guild_id
					)
				);
				false
			}
		} else {
			msg.channel_id
				.say(&ctx, "Must be in a voice channel")
				.await
				.log_err();
			false
		}
	}
	pub async fn leave(&self, _ctx: &Context, msg: &Message) -> bool {
		let guild_id = msg.guild_id.unwrap();
		self.stop(&guild_id).await;
		self.remove_queue(&guild_id).await;
		let mut lock = self.voice_manager.lock().await;
		let voice: &mut ClientVoiceManager = lock.deref_mut();
		voice.leave(guild_id).is_some()
	}
	pub async fn play_file_cmd(
		&self,
		_ctx: &Context,
		msg: &Message,
		args: &mut Args,
	) -> Result<(), &'static str> {
		let file: OsString = match args.single_quoted::<String>() {
			Ok(x) => x.into(),
			Err(_) => return Err("Usage: /play \"song name\""),
		};
		let mut lock = self.voice_manager.lock().await;
		let voice: &mut ClientVoiceManager = lock.deref_mut();
		let handler = match voice.get_mut(msg.guild_id.unwrap()) {
			Some(x) => x,
			None => {
				return Err("Must run /join first while in a voice channel");
			}
		};
		let guild_id = msg.guild_id.unwrap();
		if self.sources.read().await.contains_key(&guild_id) {
			let mut queue = self.queue.write().await;
			let guild = queue.deref_mut().get_mut(&guild_id);
			if let Some(q) = guild {
				q.write().await.push(file);
			} else {
				queue.deref_mut().insert(guild_id, RwLock::new(vec![file]));
			}
			return Ok(());
		}
		self.play_file(file, handler, msg.guild_id.unwrap()).await?;
		Ok(())
	}
	pub async fn play_file(
		&self,
		file: std::ffi::OsString,
		handler: &mut Handler,
		guild_id: GuildId,
	) -> Result<(), &'static str> {
		let mut dir = match fs::read_dir("/home/emp/Music") {
			Ok(x) => x,
			Err(e) => {
				log_timestamp!("VOICE/ERROR", format!("Failed to open music dir {}", e));
				return Err("Don't delay, ping @Emp today!");
			}
		};
		let v: Option<std::io::Result<DirEntry>> = dir.find(|x| match x {
			Ok(x) => {
				let path: PathBuf = x.path();
				if !path.is_dir() && path.file_stem().unwrap_or_default() == file {
					return true;
				}
				false
			}
			Err(e) => {
				log_timestamp!("VOICE/ERROR", format!("Failed to open music file {}", e));
				false
			}
		});
		if let Some(f) = v {
			let dir: DirEntry = f.unwrap();
			let audio: Box<dyn AudioSource> = match ffmpeg(dir.path().as_os_str()).await {
				Ok(a) => a,
				Err(e) => {
					log_timestamp!("VOICE/ERROR", format!("Failed to open ffmpeg file {}", e));
					return Err("FFmpeg died ping Emp");
				}
			};
			let audio: LockedAudio = handler.play_returning(audio);
			self.sources.write().await.insert(guild_id, (audio, file));
			Ok(())
		} else {
			Err("Invalid song name")
		}
	}
	pub async fn stop(&self, guild_id: &GuildId) {
		self.sources.write().await.remove(guild_id);
		if let Some(x) = self.voice_manager.lock().await.get_mut(guild_id) {
			x.stop()
		}
	}

	pub async fn remove_queue(&self, guild_id: &GuildId) {
		self.queue.write().await.remove(guild_id);
	}

	pub async fn remove_from_queue(&self, guild_id: &GuildId, index: usize) -> bool {
		match self.queue.read().await.get(guild_id) {
			Some(x) => {
				if x.read().await.len() > index {
					x.write().await.remove(index);
					true
				} else {
					false
				}
			}
			None => false,
		}
	}
}
#[command]
#[description = "Join your current voice channel"]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
	ctx.data
		.read()
		.await
		.get::<OofVoice>()
		.unwrap()
		.read()
		.await
		.join(ctx, msg)
		.await;
	Ok(())
}

#[command]
#[description = "Leave the voice channel"]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
	ctx.data
		.read()
		.await
		.get::<OofVoice>()
		.unwrap()
		.read()
		.await
		.leave(ctx, msg)
		.await;
	Ok(())
}

#[command]
#[description = "Play a music file"]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	if let Err(e) = ctx
		.data
		.read()
		.await
		.get::<OofVoice>()
		.unwrap()
		.read()
		.await
		.play_file_cmd(ctx, msg, &mut args)
		.await
	{
		msg.channel_id.say(&ctx, e).await.log_err();
	}
	Ok(())
}

#[command]
#[description = "Stops playing music"]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
	ctx.data
		.read()
		.await
		.get::<OofVoice>()
		.unwrap()
		.read()
		.await
		.stop(&msg.guild_id.unwrap())
		.await;
	Ok(())
}

#[command]
#[description = "Removes song from the queue"]
#[only_in(guilds)]
async fn remove(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	if let Ok(number) = args.single::<usize>() {
		if number == 0
			|| !ctx
				.data
				.read()
				.await
				.get::<OofVoice>()
				.unwrap()
				.read()
				.await
				.remove_from_queue(&msg.guild_id.unwrap(), number - 1)
				.await
		{
			msg.channel_id
				.say(&ctx, "Invalid queue number")
				.await
				.log_err();
		}
	} else {
		msg.channel_id
			.say(&ctx, "Syntax: /remove <number in queue>")
			.await
			.log_err();
	}
	Ok(())
}

#[command]
#[description = "Lists the current queue"]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message) -> CommandResult {
	let guild_id = msg.guild_id.unwrap();
	let oofvoice = ctx.data.read().await;
	let oofvoice = oofvoice.get::<OofVoice>().unwrap();
	let oofvoice = oofvoice.read().await;
	let mut message = MessageBuilder::new();
	{
		// There will always be no queue if theres no current song
		let lock = oofvoice.sources.read().await;
		let song = match lock.get(&guild_id) {
			Some(x) => x.1.to_str().unwrap(),
			None => {
				msg.channel_id.say(&ctx, "None").await.log_err();
				return Ok(());
			}
		};
		message.push_line(format!("Currently Playing: {}", song));
	}
	let queues = oofvoice.queue.read().await;
	let queue = match queues.get(&guild_id) {
		Some(x) => x.read().await,
		None => return Ok(()),
	};

	for i in queue.deref().iter().enumerate() {
		message.push_line(format!("{}: {}", i.0 + 1, i.1.to_str().unwrap()));
	}
	msg.channel_id.say(&ctx, message).await.log_err();
	Ok(())
}

#[command]
#[description = "List all available songs"]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
	let music_folder: std::fs::ReadDir = std::fs::read_dir("/home/emp/Music").unwrap();
	let mut message = MessageBuilder::new();
	for i in music_folder {
		let i: DirEntry = i.unwrap();
		if i.file_type().unwrap().is_file() {
			message.push_line(i.path().file_stem().unwrap().to_str().unwrap());
		}
	}
	msg.channel_id.say(&ctx, message).await.log_err();
	Ok(())
}
