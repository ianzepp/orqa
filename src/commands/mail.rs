//! Mail command dispatcher (thin layer over mailbox module).

use crate::cli::{CommandContext, MailCommand, MailSubcommand};
use crate::model::Orqa;

pub(crate) fn mail(
    orqa: &Orqa,
    context: &CommandContext,
    command: MailCommand,
) -> Result<(), String> {
    match command.command {
        MailSubcommand::Home(args) => {
            let fin = context.resolve_fin(args.pod, args.fin, orqa)?;
            println!("{}", orqa.mail_home(&fin)?.display());
            Ok(())
        }
        MailSubcommand::Send(args) => crate::mailbox::send_mail(orqa, context, args),
        MailSubcommand::List(args) => crate::mailbox::list_mail(orqa, context, args),
        MailSubcommand::Read(args) => crate::mailbox::read_mail(orqa, context, args),
        MailSubcommand::Done(args) => crate::mailbox::done_mail(orqa, context, args),
        MailSubcommand::Delete(args) => crate::mailbox::delete_mail(orqa, context, args),
        MailSubcommand::Unread(args) => crate::mailbox::unread_mail(orqa, context, args),
    }
}
