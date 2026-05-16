use clap::{Args, Subcommand};

use super::fin::FinRefArgs;

#[derive(Debug, Args)]
pub(crate) struct MailMessageArgs {
    /// Message id, filename, or path.
    pub(crate) message: String,
}
#[derive(Debug, Args)]
pub(crate) struct MailListArgs {
    /// Include done items from cur.
    #[arg(long)]
    pub(crate) all: bool,
}
#[derive(Debug, Args)]
pub(crate) struct SendMailArgs {
    /// Sender address. Defaults to ORQA_FIN@ORQA_POD.orqa.
    #[arg(long)]
    pub(crate) from: Option<String>,
    /// Recipient address, such as bob-jones or bob-jones@sample-pod.orqa.
    #[arg(long)]
    pub(crate) to: String,
    /// Message subject.
    #[arg(long, default_value = "(no subject)")]
    pub(crate) subject: String,
    /// Message body. Reads stdin when omitted.
    pub(crate) body: Option<String>,
}
#[derive(Debug, Args)]
pub(crate) struct MailCommand {
    #[command(subcommand)]
    pub(crate) command: MailSubcommand,
}
#[derive(Debug, Subcommand)]
pub(crate) enum MailSubcommand {
    /// Print the mail directory for a fin.
    Home(FinRefArgs),
    /// Send mail.
    Send(SendMailArgs),
    /// List mail.
    List(MailListArgs),
    /// Read mail.
    Read(MailMessageArgs),
    /// Mark mail done.
    Done(MailMessageArgs),
    /// Delete mail.
    Delete(MailMessageArgs),
    /// List unread mail.
    Unread(FinRefArgs),
}
