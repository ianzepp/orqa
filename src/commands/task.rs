//! Task command dispatcher (thin layer over mailbox module).

use crate::cli::{TaskCommand, TaskSubcommand};
use crate::model::Orqa;

pub(crate) fn task(orqa: &Orqa, command: TaskCommand) -> Result<(), String> {
    match command.command {
        TaskSubcommand::Home(args) => {
            let fin = crate::model::FinRef::new(&args.pod, &args.fin)?;
            println!("{}", orqa.task_home(&fin).display());
            Ok(())
        }
        TaskSubcommand::Send(args) => crate::mailbox::send_task(orqa, args),
        TaskSubcommand::List(args) => crate::mailbox::list_tasks(orqa, args),
        TaskSubcommand::Read(args) => crate::mailbox::read_item(orqa, args, crate::mailbox::ItemKind::Task),
        TaskSubcommand::Done(args) => crate::mailbox::done_item(orqa, args, crate::mailbox::ItemKind::Task),
        TaskSubcommand::Delete(args) => crate::mailbox::delete_item(orqa, args, crate::mailbox::ItemKind::Task),
    }
}
