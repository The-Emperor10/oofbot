use std::collections::HashMap;

use serenity::client::Context;
use serenity::model::guild::Member;
use serenity::model::id::GuildId;
use serenity::model::id::UserId;

pub async fn get_guild_members(ctx: &Context, guild: GuildId) -> Option<Vec<Member>> {
	if let Some(guild) = ctx.cache.guild(guild).await {
		Some(guild.members.values().cloned().collect())
	} else if let Ok(guild) = guild.to_partial_guild(&ctx).await {
		guild.members(&ctx, None, None).await.ok()
	} else {
		None
	}
}
#[macro_export]
macro_rules! log {
	($tag:expr, $($message:expr),+) => {{
		use std::io::Write;
		let file = std::fs::OpenOptions::new().append(true).read(false).create(true).truncate(false).open("oofbot.log");
		$(
			let message = format!("[{}] {}", $tag.to_string(), $message.to_string());
			if let Ok(mut f) = file {
				writeln!(f, "{}", message).unwrap();
			}
			println!("{}", message);
		)+
	}}
}
#[macro_export]
macro_rules! log_timestamp {
	($tag:expr, $($message:expr),+) => {{
		use std::io::Write;
		use chrono::prelude::*;
		let time: DateTime<Local> = Local::now();
		let file = std::fs::OpenOptions::new().append(true).read(false).create(true).truncate(false).open("oofbot.log");
		$(
			let msg: String = format!("[{hh:02}:{mm:02}:{ss:02}][{tag}] {message}",
				tag=$tag,
				message=$message,
				hh=time.hour(),
				mm=time.minute(),
				ss=time.second()
			);
			if let Ok(mut f) = file {
				writeln!(f, "{}", msg).unwrap();
			}
			println!("{}", msg);
		)+
	}};
}
