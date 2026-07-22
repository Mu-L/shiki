mod commands;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use shiki_config::Config;
use shiki_core::NotebookStore;

#[derive(Parser)]
#[command(
    name = "shiki",
    version,
    about = "shiki (私記) — personal notes in the terminal"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Creates a new note and opens $EDITOR
    New {
        title: String,
        #[arg(short, long)]
        notebook: Option<String>,
    },
    /// Lists the notes in a notebook
    List {
        #[arg(short = 'n', long)]
        notebook: Option<String>,
    },
    /// Edits a note with $EDITOR
    Edit {
        note: String,
        #[arg(short, long)]
        notebook: Option<String>,
    },
    /// Shows the rendered contents of a note
    Show {
        note: String,
        #[arg(short, long)]
        notebook: Option<String>,
    },
    /// Searches notes by title (fuzzy)
    Search {
        query: String,
        #[arg(short, long)]
        notebook: Option<String>,
    },
    /// Creates or opens today's daily note
    Daily {
        #[arg(short, long)]
        notebook: Option<String>,
    },
    /// Git commit (+ push if enabled) for a notebook
    Sync {
        #[arg(short = 'n', long)]
        notebook: Option<String>,
    },
    /// Shows the path to the config file
    Config,
    /// Checks the environment (config, data dir, git, editor, terminal, notebooks)
    Doctor,
    /// Manages notebooks
    Notebook {
        #[command(subcommand)]
        action: NotebookAction,
    },
    /// Lists or switches the color theme
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
    },
}

#[derive(Subcommand)]
enum NotebookAction {
    Create { name: String },
    List,
    Rename { old: String, new: String },
}

#[derive(Subcommand)]
enum ThemeAction {
    /// Lists all built-in themes, marking the active one
    List,
    /// Sets the active theme by name (see `shiki theme list`)
    Set { name: String },
}

struct Context {
    config: Config,
    store: NotebookStore,
}

impl Context {
    fn load() -> Result<Self> {
        let config_path = Config::default_path()?;
        let config = Config::load_or_init(&config_path)?;
        let templates_dir = Config::default_templates_dir()?;
        shiki_core::templates::ensure_defaults(&templates_dir)?;
        let data_dir = Config::default_data_dir()?;
        let store = NotebookStore::new(data_dir);
        Ok(Self { config, store })
    }

    fn notebook_name(&self, override_name: Option<String>) -> String {
        override_name.unwrap_or_else(|| self.config.general.default_notebook.clone())
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    // Handled before `Context::load()` — doctor needs to work (and say why)
    // even when the config is broken, which is precisely the situation
    // someone reaching for it is usually in.
    if matches!(cli.command, Some(Commands::Doctor)) {
        return commands::doctor::run();
    }

    let mut ctx = Context::load()?;

    match cli.command {
        None => tui::launch(ctx.config, ctx.store),
        Some(Commands::New { title, notebook }) => {
            let notebook = ctx.notebook_name(notebook);
            commands::new::run(&ctx.store, &notebook, &title, &ctx.config.general.editor)
        }
        Some(Commands::List { notebook }) => {
            let notebook = ctx.notebook_name(notebook);
            commands::list::run(&ctx.store, &notebook)
        }
        Some(Commands::Edit { note, notebook }) => {
            let notebook = ctx.notebook_name(notebook);
            commands::edit::run(&ctx.store, &notebook, &note, &ctx.config.general.editor)
        }
        Some(Commands::Show { note, notebook }) => {
            let notebook = ctx.notebook_name(notebook);
            commands::show::run(&ctx.store, &notebook, &note)
        }
        Some(Commands::Search { query, notebook }) => {
            let notebook = ctx.notebook_name(notebook);
            commands::search::run(&ctx.store, &notebook, &query)
        }
        Some(Commands::Daily { notebook }) => {
            let notebook = ctx.notebook_name(notebook);
            let templates_dir = Config::default_templates_dir()?;
            commands::daily::run(
                &ctx.store,
                &notebook,
                &templates_dir,
                &ctx.config.general.editor,
            )
        }
        Some(Commands::Sync { notebook }) => {
            let notebook = ctx.notebook_name(notebook);
            commands::sync::run(&ctx.store, &notebook, &ctx.config.git)
        }
        Some(Commands::Config) => commands::config::run(),
        Some(Commands::Doctor) => unreachable!("handled before Context::load() above"),
        Some(Commands::Notebook { action }) => match action {
            NotebookAction::Create { name } => commands::notebook::create(&ctx.store, &name),
            NotebookAction::List => commands::notebook::list(&ctx.store),
            NotebookAction::Rename { old, new } => {
                commands::notebook::rename(&ctx.store, &old, &new)
            }
        },
        Some(Commands::Theme { action }) => match action {
            ThemeAction::List => commands::theme::list(&ctx.config),
            ThemeAction::Set { name } => commands::theme::set(&mut ctx.config, &name),
        },
    }
}
