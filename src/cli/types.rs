use anyhow::Context;
use clap::{App, AppSettings, Arg, ArgMatches};
use thiserror::Error;

use std::any::Any;
use std::collections::HashMap;

use crate::cli::doc::cmdln_pager::show_doc_page;

#[derive(Error, Debug)]
/// Lucky CLI error variants
pub(crate) enum CliError {
    #[error("Process exiting with code: {0}")]
    /// Indicates that the process should exit with the given code
    Exit(i32),
}

pub(crate) type CliData = HashMap<String, Box<dyn Any>>;

/// Trait for Lucky commands and subcommands
///
/// Commands in the Lucky CLI should implement this trait
pub(crate) trait CliCommand<'a>: CliCommandExt<'a> {
    /// This should return the name of the subcommand
    fn get_name(&self) -> &'static str;
    /// This should use `get_base_app("command_name")` to create a clap app and then use the
    /// builder to modify it. Subcommands should not be added to the app. To add subcommands
    /// you should return boxed `CliCommand`'s from `get_subcommands()`.
    fn get_app(&self) -> App<'a>;
    /// This should return a `Vec` of boxed `CliCommand`'s. `get_cli()` will automatically add
    /// these to the app returned by `get_command()`.
    fn get_subcommands(&self) -> Vec<Box<dyn CliCommand<'a>>>;
    /// This should return the markdown template for the command's documentation.
    fn get_doc(&self) -> Option<CliDoc>;
    /// This should run any code that should be executed when the command is executed. If this
    /// command has subcommands, the selected subcommand will run with the output of this function
    /// being passed to the subcommands `execute_command`.
    ///
    /// The `data` value is meant to allow subcommands to recieve data from their parent commands
    /// and the return value is to allow parent commands to pass the data to the subcommand.
    fn execute_command(&self, args: &ArgMatches, data: CliData) -> anyhow::Result<CliData>;
}

/// Extension trait to the `CliCommand` trait
///
/// This trait has a blanket implementation on top of all `CliCommands`, providing implementations
/// of extra functions required by the CLI.
pub(crate) trait CliCommandExt<'a> {
    /// Return the clap app for this command
    fn get_cli(&self) -> App<'a>;
    /// Run the command arbitrary data can be passed in the `data` argument
    fn run(&self, args: &ArgMatches, data: CliData) -> anyhow::Result<()>;
    /// Creates a clap app with our default settings. This should be used by implementors to
    /// create a base app when implementing `get_command()`.
    fn get_base_app(&self) -> App<'a>;
}

impl<'a, C: CliCommand<'a>> CliCommandExt<'a> for C {
    fn get_cli(&self) -> App<'a> {
        let mut cmd = self.get_app();

        for subcommand in Self::get_subcommands(self) {
            cmd = cmd.subcommand(subcommand.get_cli());
        }

        cmd
    }

    fn run(&self, args: &ArgMatches, data: CliData) -> anyhow::Result<()> {
        // Check for the --doc flag and show the doc page if present
        if args.is_present("doc") {
            show_doc_page(self).context("Could not show doc page")?;
        }

        // Run the command
        let out_data = self.execute_command(args, data)?;

        // Run the selected subcommand if any
        if let (subcmd_name, Some(args)) = args.subcommand() {
            for subcommand in self.get_subcommands() {
                if subcommand.get_name() == subcmd_name {
                    return subcommand.run(args, out_data);
                }
            }
        }

        Ok(())
    }

    #[rustfmt::skip]
    fn get_base_app(&self) -> App<'a> {
        App::new(self.get_name())
            // Set the max term width the 3 short of  the actual width so that we don't wrap
            // on the help pager. Width is 3 shorter because of 1 char for the scrollbar and
            // 1 char padding on each side.
            .max_term_width(
                crossterm::terminal::size()
                    .map(|size| size.0 - 3)
                    .unwrap_or(0) as usize,
            )
            .setting(AppSettings::ColoredHelp)
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::ArgRequiredElseHelp)
            .setting(AppSettings::DisableHelpSubcommand)
            .mut_arg("help", |arg| {
                arg.short('h')
                    .long("help")
                    .help("-h: show short help, --help: show long help")
            })
            .arg(Arg::with_name("doc")
                .help(match self.get_doc() {
                    Some(_) => "Show the detailed command documentation ( similar to a man page )",
                    None => "Does nothing for this command: this command does not have a doc page"
                })
                .long("doc")
                .short('H'))
    }
}

#[derive(Debug)]
/// The documentation for a CLI command
pub struct CliDoc {
    /// The name of the doc page, used to store the scrolled location in the doc
    pub name: &'static str,
    /// The documentation content
    pub content: &'static str,
}
