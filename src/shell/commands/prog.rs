use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use wasmi::{Caller, Engine, Linker, Module, Store};
use crate::fs::read_file;
use crate::println;
use crate::shell::commands::Command;
use crate::shell::prompt;
use crate::wasm::run;
use crate::task::executor::{spawn_task, SUPPRESS_PROMPT};
use crate::task::Task;

pub struct RunCommand;
impl Command for RunCommand {
    fn name(&self) -> &'static str { "run" }
    fn description(&self) -> &'static str { "Run a program" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() {
            println!("Usage: run <filename>");
            return;
        }
        let data = match read_file(&args[0]) {
            Some(data) => data,
            None => {
                println!("Failed to read {}", args[0]);
                return;
            }
        };

        // Run the program as its own async task so the executor can
        // interleave it with other tasks. Suppress the shell's immediate
        // re-prompt; the task re-prompts when the program exits.
        SUPPRESS_PROMPT.store(true, Ordering::SeqCst);
        spawn_task(Task::new(async move {
            match run(data).await {
                Ok(()) => println!("Program completed"),
                Err(e) => println!("Error during program execution: {}", e),
            }
            SUPPRESS_PROMPT.store(false, Ordering::SeqCst);
            prompt();
        }));
    }
}