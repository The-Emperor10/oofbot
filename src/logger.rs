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
