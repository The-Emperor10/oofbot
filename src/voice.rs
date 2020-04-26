use serenity::model::id::GuildId;
use std::collections::HashMap;
use std::fs::DirEntry;
use std::path::PathBuf;
use serenity::{
	model::{
		channel::{Message}
	},
	prelude::*,
	framework::standard::{
		*,
		macros::{command, group}
	},
	client::bridge::voice::*,
	voice::*,
	utils::MessageBuilder
};
use std::{sync::{Arc}, ops::{Deref, DerefMut}, fs, thread, time::Duration, ffi::OsString};
use crate::{LogResult};

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
	pub queue: Arc<RwLock<HashMap<GuildId, RwLock<Vec<OsString>>>>>
}

impl OofVoice {
	pub fn new(voice_manager: Arc<SerMutex<ClientVoiceManager>>, framework: &mut StandardFramework) -> Arc<RwLock<Self>> {
		framework.group_add(&VOICE_GROUP);
		let s = Arc::new(RwLock::new(Self {voice_manager, sources: Default::default(), queue: Default::default()}));
		let oofvoice = s.clone();
        
        // Spaghetti warning
		thread::spawn(move || {
			loop {
				thread::sleep(Duration::from_secs(5));
				let oof = oofvoice.read();
				let sources = oof.sources.read();
				let mut next: Vec<(GuildId, Option<OsString>)> = Vec::with_capacity(sources.deref().len());
				for i in sources.deref() {
					if (i.1).0.lock().finished {
						next.push((i.0.clone(), None));
					}
				}
				drop(sources);
				let queue = oof.queue.read();
				
				for mut i in &mut next {
					if let Some(vec) = queue.get(&i.0) {
						let mut vec = vec.write();
						if vec.len() > 0 {
							i.1 = Some(vec.remove(0));
						}
					}
				}
				drop(queue);
				for i in next {
					if let Some(queued) = i.1 {
						let handler;
						let mut lock = oof.voice_manager.lock();
						let voice: &mut ClientVoiceManager = lock.deref_mut();
						handler = match voice.get_mut(i.0) {
							Some(x) => x,
							None => continue
						};
						oof.play_file(queued, handler, i.0).log_err();
					}
				}
			}
		});
		s
	}
    pub fn join(&self, ctx: &Context, msg: &Message) -> bool {
		let guild_id = msg.guild_id.unwrap();
		let mut lock = self.voice_manager.lock();
		let voice: &mut ClientVoiceManager = lock.deref_mut();

		let glock1 = msg.guild(&ctx).unwrap();
		let glock2 = glock1.read();
		let guild = glock2.deref();

		let channel_id = guild.voice_states.get(&msg.author.id).and_then(|voice_state| voice_state.channel_id);
		if let Some(channel) = channel_id {
			if let Some(_) = voice.join(guild_id, channel) {
				log_timestamp!("VOICE/INFO", format!("Joined voice channel {} in guild {}", channel, guild_id));
				return true;
			}
			else {
				msg.channel_id.say(&ctx, "Failed to join the channel").log_err();
				log_timestamp!("VOICE/ERROR", format!("Failed to join voice channel {} in guild {}", channel, guild_id));
				return false;
			}
		}
		else {
			msg.channel_id.say(&ctx, "Must be in a voice channel").log_err();
			return false;
		}
    }
    pub fn leave(&self, _ctx: &Context, msg: &Message) -> bool {
		let guild_id = msg.guild_id.unwrap();
		self.stop(&guild_id);
		self.remove_queue(&guild_id);
		let mut lock = self.voice_manager.lock();
		let voice: &mut ClientVoiceManager = lock.deref_mut();
		voice.leave(guild_id).is_some()
    }
	pub fn play_file_cmd(&self, _ctx: &Context, msg: &Message, args: &mut Args) -> Result<(), &'static str> {
        let file: OsString = match args.single_quoted::<String>() {
            Ok(x) => x.into(),
            Err(_) => return Err("Usage: /play \"song name\"")
        };
		let mut lock = self.voice_manager.lock();
		let voice: &mut ClientVoiceManager = lock.deref_mut();
		let handler = match voice.get_mut(msg.guild_id.unwrap()) {
			Some(x) => x,
			None => {
				return Err("Must run /join first while in a voice channel");
			}
		};
		let guild_id = msg.guild_id.unwrap();
		if self.sources.read().contains_key(&guild_id) {
			let mut queue = self.queue.write();
			let guild = queue.deref_mut().get_mut(&guild_id);
			if let Some(q) = guild {
				q.write().push(file);
			}
			else {
				queue.deref_mut().insert(guild_id, RwLock::new(vec!(file)));
			}
			return Ok(());
		}
		self.play_file(file.into(), handler, msg.guild_id.unwrap())?;
        Ok(())
	}
	pub fn play_file(&self, file: std::ffi::OsString, handler: &mut Handler, guild_id: GuildId) -> Result<(), &'static str> {
		let dir = match fs::read_dir("/home/emp/Music") {
			Ok(x) => x, 
			Err(e) => {
				log_timestamp!("VOICE/ERROR", format!("Failed to open music dir {}", e)); 
				return Err("Emp broke a thing ping him");
			}
		};
		let v: Option<std::io::Result<DirEntry>> = dir.filter(|x| {
			match x {
				Ok(x) => {
					let path: PathBuf = x.path();
					if !path.is_dir() && path.file_stem().unwrap_or_default() == file {
						return true;
					}
					false
				},
				Err(e) => {
					log_timestamp!("VOICE/ERROR", format!("Failed to open music file {}", e)); 
					false
				}
			}
		}).next();
		if let Some(f) = v {
			let dir: DirEntry = f.unwrap();
			let audio: Box<dyn AudioSource> = match ffmpeg(dir.path().as_os_str()) {
				Ok(a) => a,
				Err(e) => {
					log_timestamp!("VOICE/ERROR", format!("Failed to open ffmpeg file {}", e)); 
					return Err("FFmpeg died ping Emp");
				}
			};
			let audio: LockedAudio = handler.play_returning(audio);
			self.sources.write().insert(guild_id, (audio, file));
            Ok(())
		}
		else {
			return Err("Invalid song name");
		}
	}
	pub fn stop(&self, guild_id: &GuildId) {
		self.sources.write().remove(guild_id);
		match self.voice_manager.lock().get_mut(guild_id) {
			Some(x) => x.stop(),
			None => return
		};
	}

	pub fn remove_queue(&self, guild_id: &GuildId) {
		self.queue.write().remove(guild_id);
	}
	
	pub fn remove_from_queue(&self, guild_id: &GuildId, index: usize) -> bool {
		match self.queue.read().get(guild_id) {
			Some(x) => {
				if x.read().len() > index {
					x.write().remove(index);
					true
				}
				else {
					false
				}
			},
			None => false
		}
	}
}
#[command]
#[description = "Join your current voice channel"]
#[only_in(guilds)]
fn join(ctx: &mut Context, msg: &Message) -> CommandResult {
	ctx.data.read().get::<OofVoice>().cloned().unwrap().read().join(ctx, msg);
	Ok(())
}

#[command]
#[description = "Leave the voice channel"]
#[only_in(guilds)]
fn leave(ctx: &mut Context, msg: &Message) -> CommandResult {
	ctx.data.read().get::<OofVoice>().cloned().unwrap().read().leave(ctx, msg);
	Ok(())
}

#[command]
#[description = "Play a music file"]
#[only_in(guilds)]
fn play(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    if let Err(e) = ctx.data.read().get::<OofVoice>().cloned().unwrap().read().play_file_cmd(ctx,msg,&mut args) {
        msg.channel_id.say(&ctx, e).log_err();
    }
	Ok(())
}

#[command]
#[description = "Stops playing music"]
#[only_in(guilds)]
fn stop(ctx: &mut Context, msg: &Message) -> CommandResult {
	ctx.data.read().get::<OofVoice>().cloned().unwrap().read().stop(&msg.guild_id.unwrap());
	Ok(())
}

#[command]
#[description = "Removes song from the queue"]
#[only_in(guilds)]
fn remove(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
	if let Ok(number) = args.single::<usize>() {
		if number == 0 || !ctx.data.read().get::<OofVoice>().cloned().unwrap().read().remove_from_queue(&msg.guild_id.unwrap(), number-1) {
			msg.channel_id.say(&ctx, "Invalid queue number").log_err();
		}
	}
	else {
		msg.channel_id.say(&ctx, "Syntax: /remove <number in queue>").log_err();
	}
	Ok(())
}

#[command]
#[description = "Lists the current queue"]
#[only_in(guilds)]
fn queue(ctx: &mut Context, msg: &Message) -> CommandResult {
	let guild_id = msg.guild_id.unwrap();
	let _oofvoice = ctx.data.read().get::<OofVoice>().cloned().unwrap();
	let oofvoice = _oofvoice.read();
	let mut message = MessageBuilder::new();
	{
		// There will always be no queue if theres no current song
		let lock = oofvoice.sources.read();
		let song = match lock.get(&guild_id) {
			Some(x) => x.1.to_str().unwrap(),
			None => {msg.channel_id.say(&ctx, "None").log_err(); return Ok(());}
		};
		message.push_line(format!("Currently Playing: {}",song));
	}
	let queues = oofvoice.queue.read();
	let queue = match queues.get(&guild_id) {
		Some(x) => x.read(),
		None => return Ok(())
	};
	
	for i in queue.deref().iter().enumerate() {
		message.push_line(format!("{}: {}", i.0+1, i.1.to_str().unwrap()));
	}
	msg.channel_id.say(&ctx, message).log_err();
	Ok(())
}

#[command]
#[description = "List all available songs"]
fn list(ctx: &mut Context, msg: &Message) -> CommandResult {
	let music_folder: std::fs::ReadDir = std::fs::read_dir("/home/emp/Music").unwrap();
	let mut message = MessageBuilder::new();
	for i in music_folder {
		let i: DirEntry = i.unwrap();
		if i.file_type().unwrap().is_file() {
			message.push_line(i.path().file_stem().unwrap().to_str().unwrap());
		}
	}
	msg.channel_id.say(&ctx, message).log_err();
	Ok(())
}
