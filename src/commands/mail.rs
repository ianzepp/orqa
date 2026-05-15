//! Mail command dispatcher (thin layer over mailbox module).

use crate::cli::{MailCommand, MailSubcommand};
use crate::model::Orqa;

pub(crate) fn mail(orqa: &Orqa, command: MailCommand) -> Result<(), String> {
    match command.command {
        MailSubcommand::Home(args) => {
            let fin = crate::model::FinRef::new(&args.pod, &args.fin)?;
            println!("{}", orqa.mail_home(&fin).display());
            Ok(())
        }
        MailSubcommand::Send(args) => crate::mailbox::send_mail(orqa, args),
        MailSubcommand::List(args) => crate::mailbox::list_mail(orqa, args),
        MailSubcommand::Read(args) => crate::mailbox::read_mail(orqa, args),
        MailSubcommand::Done(args) => crate::mailbox::done_mail(orqa, args),
        MailSubcommand::Delete(args) => crate::mailbox::delete_mail(orqa, args),
        MailSubcommand::Unread(args) => crate::mailbox::unread_mail(orqa, args),
    }
}
