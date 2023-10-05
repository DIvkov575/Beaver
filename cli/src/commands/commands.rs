
#[derive(Parser, Debug)]
pub enum Command {
    /// initialize beaver config dir
    Init(),
    /// Tmp
    Tmp(),
    /// Generates manpages
    Manpage(),
}
impl Command {
    pub fn run(self, config_args: &super::ConfigArgs) -> Result<()> {
        use Command::*;
        match self {
            List(args) => args.run(config_args),
            Put(args) => args.run(config_args),
            Empty(args) => args.run(config_args),
            Restore(args) => args.run(config_args),
            Completions(args) => args.run(),
            Manpage(args) => args.run(),
        }
    }
}