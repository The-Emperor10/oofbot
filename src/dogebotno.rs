use std::{
	thread,
	sync::{
		atomic::{
			Ordering
		}
	},
	time::Duration,
	ops::Deref
};
use crate::Handler;
use crate::canary_update::CanaryUpdateHandler;
use crate::LogResult;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::utils::MessageBuilder;


pub fn dogebotno(ctx: Context, msg: Message) {
	if msg.content.contains("(offline)") {
		msg.channel_id.say(&ctx, "rip").log_err();
		return;
	}
	let _ = match msg.content.as_str() {
		"^" => {
			msg.channel_id.say(&ctx, "^").unwrap();
			return;
		},
		"its treason then" => {
			msg.channel_id.say(&ctx, "Mace went out of the windu").unwrap();
			return;
		},
		_ => ()
	};
	for i in msg.attachments {
		let _ = match i.filename.as_str() {
			"trollface.jpg" => {
				msg.channel_id.say(&ctx, "I see no problems here <@612070962913083405>").unwrap();
				return;
			},
			"creeper.jpg" => {
				msg.channel_id.say(&ctx, "its fine its fine, it doesnt bother me, it doesnt bother me, IT BOTHERS ME, IT BOTHERS ME A LOT\n||THAT THAT ONES NOT GREEN||").unwrap();
				return;
			},
			_ => ()
		};
	}
	return;
}

pub fn dogebot_presence(ctx: &Context, data: &PresenceUpdateEvent, guild_id: &GuildId, handler: &Handler) {
    if data.presence.status == OnlineStatus::Offline {
	    let canary = ctx.data.read().get::<CanaryUpdateHandler>().cloned().unwrap();
	    let lock = canary.lock().unwrap();
	    let channel = lock.get_update_channel(&guild_id);
	    drop(lock);
	    if let Some(channel) = channel {
	    	log_timestamp!("INFO", "Dogebot went offline");
	    	let ctp = handler.cancel_tyler_ping.clone();
	    	let ctx = ctx.http.clone();
	    	thread::spawn(move || {
                log_timestamp!("DEBUG", "Dogebot thread started");
	    		thread::sleep(Duration::from_secs(5));

	    		let mut v = ctp.load(Ordering::SeqCst);
	    		if v {
                    log_timestamp!("DEBUG", "Returning from dogebot thread before message");
	    			return ctp.store(false, Ordering::SeqCst);
	    		}

	    		log_timestamp!("INFO", "Dogebot went completely offline");
	    		channel.say(&ctx, "dogebot is offline. Everyone press F to pay respects, and press <:thisisfine:667895278535311381> for another ~~bug~~ feature!").log_err();

	    		for _ in 0..58 {
	    			thread::sleep(Duration::from_secs(5));
	    			v = ctp.load(Ordering::SeqCst);
	    			if v {
                        log_timestamp!("DEBUG", "Returning from dogebot thread before pinging tyler");
	    				return ctp.store(false, Ordering::SeqCst);
	    			}
	    		}
                ctp.store(false, Ordering::SeqCst);
	    		let msg = MessageBuilder::new().user(355803584228622346).build();
	    		channel.say(&ctx, msg).log_err()
	    	});
	    	return;
	    }
	    else {
	    	log_timestamp!("WARN", "No canary update channel set for DVS");
	    }
    }
    else if data.presence.status == OnlineStatus::Online {
	    log_timestamp!("INFO", "Dogebot back");
	    handler.cancel_tyler_ping.deref().store(true, Ordering::SeqCst);
    }
}
