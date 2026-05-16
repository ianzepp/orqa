//! Task command dispatcher (thin layer over mailbox module).

use crate::cli::{CommandContext, TaskCommand, TaskSubcommand};
use crate::model::Orqa;

pub(crate) fn task(
    orqa: &Orqa,
    context: &CommandContext,
    command: TaskCommand,
) -> Result<(), String> {
    match command.command {
        TaskSubcommand::Home(args) => {
            let fin = context.resolve_fin(None, args.fin, orqa)?;
            println!("{}", orqa.task_home(&fin)?.display());
            Ok(())
        }
        TaskSubcommand::Send(args) => crate::mailbox::send_task(orqa, context, args),
        TaskSubcommand::List(args) => crate::mailbox::list_tasks(orqa, context, args),
        TaskSubcommand::Read(args) => {
            crate::mailbox::read_item(orqa, context, args, crate::mailbox::ItemKind::Task)
        }
        TaskSubcommand::Done(args) => {
            crate::mailbox::done_item(orqa, context, args, crate::mailbox::ItemKind::Task)
        }
        TaskSubcommand::Delete(args) => {
            crate::mailbox::delete_item(orqa, context, args, crate::mailbox::ItemKind::Task)
        }
    }
}
