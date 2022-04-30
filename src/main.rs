use clap::{ArgEnum, Parser};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

mod old_and_bad;

// This is actually from Wikipedia, but it should do the trick
pub(crate) const CONFIGURATION_SOURCE: parse_wiki_text::ConfigurationSource =
    parse_wiki_text::ConfigurationSource {
        category_namespaces: &["category"],
        extension_tags: &[
            "categorytree",
            "ce",
            "charinsert",
            "chem",
            "gallery",
            "graph",
            "hiero",
            "imagemap",
            "indicator",
            "inputbox",
            "langconvert",
            "mapframe",
            "maplink",
            "math",
            "nowiki",
            "poem",
            "pre",
            "ref",
            "references",
            "score",
            "section",
            "source",
            "syntaxhighlight",
            "templatedata",
            "templatestyles",
            "timeline",
        ],
        file_namespaces: &["file", "image"],
        link_trail: "abcdefghijklmnopqrstuvwxyz",
        magic_words: &[
            "disambig",
            "expected_unconnected_page",
            "expectunusedcategory",
            "forcetoc",
            "hiddencat",
            "index",
            "newsectionlink",
            "nocc",
            "nocontentconvert",
            "noeditsection",
            "nogallery",
            "noglobal",
            "noindex",
            "nonewsectionlink",
            "notc",
            "notitleconvert",
            "notoc",
            "staticredirect",
            "toc",
        ],
        protocols: &[
            "//",
            "bitcoin:",
            "ftp://",
            "ftps://",
            "geo:",
            "git://",
            "gopher://",
            "http://",
            "https://",
            "irc://",
            "ircs://",
            "magnet:",
            "mailto:",
            "mms://",
            "news:",
            "nntp://",
            "redis://",
            "sftp://",
            "sip:",
            "sips:",
            "sms:",
            "ssh://",
            "svn://",
            "tel:",
            "telnet://",
            "urn:",
            "worldwind://",
            "xmpp:",
        ],
        redirect_magic_words: &["redirect"],
    };

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum, Debug)]
enum Mode {
    Wikitext,
    Markdown,
    OldAndBad,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Input database (must be a SQLite3 database)
    #[clap(short, long)]
    database_path: PathBuf,

    /// Directory to output to
    #[clap(short, long)]
    output_directory: PathBuf,

    /// What mode to run the program in
    #[clap(arg_enum, short, long)]
    mode: Mode,
}

fn parse_args() -> anyhow::Result<Args> {
    let args = Args::parse();

    let database_path = dunce::canonicalize(args.database_path)?;
    assert!(database_path.is_file());

    fs::create_dir_all(&args.output_directory)?;
    let output_directory = dunce::canonicalize(args.output_directory)?;

    Ok(Args {
        database_path,
        output_directory,
        ..args
    })
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let wiki_parser = parse_wiki_text::Configuration::new(&CONFIGURATION_SOURCE);

    let conn = rusqlite::Connection::open(args.database_path)?;
    let mut stmt = conn.prepare(
        r#"
        SELECT
            page.page_title,
            text.old_text
        FROM page
        LEFT JOIN revision ON revision.rev_id = page.page_latest
        LEFT JOIN text ON text.old_id = revision.rev_text_id
		WHERE NOT EXISTS (SELECT 1 FROM user WHERE user.user_name IS page.page_title)
        ORDER BY page.page_title COLLATE NOCASE;
    "#,
    )?;

    for (title, text) in stmt
        .query_map([], |r| -> Result<(String, String), _> {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(Result::ok)
        .filter(|(title, _)| Path::new(title).extension().is_none())
        .filter(|(title, _)| !title.to_lowercase().contains("/sandbox"))
        .filter(|(title, _)| !title.starts_with("''"))
    {
        write_file(&wiki_parser, args.mode, &args.output_directory, title, text)?;
    }

    Ok(())
}

fn write_file(
    wiki_parser: &parse_wiki_text::Configuration,
    mode: Mode,
    output_directory: &Path,
    title: String,
    text: String,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let components: Vec<_> = title.split('/').collect();

    let path: PathBuf = std::iter::once(output_directory)
        .chain(components.iter().map(Path::new))
        .collect::<PathBuf>();

    fs::create_dir_all(path.parent().context("failed to get parent path")?)?;

    match mode {
        Mode::Wikitext => {
            fs::write(path.with_extension("wikitext"), text)?;
        }
        Mode::Markdown => todo!(),
        Mode::OldAndBad => {
            let path = path.with_extension("md");
            let mut file = File::create(path)?;
            writeln!(file, "# {}\n", title)?;

            let ast = wiki_parser.parse(&text);

            let written = old_and_bad::write_nodes_with_affix(
                &mut file,
                ast.nodes.iter(),
                |_| Ok(()),
                |f| writeln!(f),
            )?;

            if !written {
                writeln!(file)?;
                writeln!(file, "```")?;
                for node in &ast.nodes {
                    writeln!(file, "{:?}", node)?;
                }
                writeln!(file, "```")?;
            }
        }
    }

    Ok(())
}
