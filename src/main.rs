use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

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

fn parse_args() -> anyhow::Result<(PathBuf, PathBuf)> {
    use clap::Parser;

    #[derive(Parser, Debug)]
    #[clap(author, version, about, long_about = None)]
    struct Args {
        /// Input database (must be a SQLite3 database)
        #[clap(short, long)]
        database_path: String,

        /// Directory to output to
        output_directory: String,
    }

    let args = Args::parse();

    let database_path = dunce::canonicalize(args.database_path)?;
    assert!(database_path.is_file());

    std::fs::create_dir_all(&args.output_directory)?;
    let output_directory = dunce::canonicalize(args.output_directory)?;

    Ok((database_path, output_directory))
}

fn main() -> anyhow::Result<()> {
    let (database_path, output_directory) = parse_args()?;
    let wiki_parser = parse_wiki_text::Configuration::new(&CONFIGURATION_SOURCE);

    let conn = rusqlite::Connection::open(database_path)?;
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
        write_file(&wiki_parser, &output_directory, title, text)?;
    }

    Ok(())
}

fn write_file(
    wiki_parser: &parse_wiki_text::Configuration,
    output_directory: &Path,
    title: String,
    text: String,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let components: Vec<_> = title.split('/').collect();

    let path: PathBuf = std::iter::once(output_directory)
        .chain(components.iter().map(Path::new))
        .collect::<PathBuf>()
        .with_extension("md");

    std::fs::create_dir_all(path.parent().context("failed to get parent path")?)?;
    let mut file = File::create(path)?;
    writeln!(file, "# {}\n", title)?;

    let ast = wiki_parser.parse(&text);
    let written = write_nodes(&mut file, ast.nodes.iter())?;

    if !written {
        writeln!(file)?;
        writeln!(file)?;
        writeln!(file)?;
        for node in &ast.nodes {
            writeln!(file, "{:?}", node)?;
        }
    }

    Ok(())
}

fn write_node(file: &mut File, node: &parse_wiki_text::Node) -> anyhow::Result<bool> {
    use parse_wiki_text::Node::*;

    #[allow(unused_variables)]
    match node {
        Heading { level, nodes, .. } => {
            write!(file, "{} ", "#".repeat(*level as usize))?;
            let result = write_nodes(file, nodes.iter());
            writeln!(file)?;
            writeln!(file)?;
            result
        }
        Link { target, text, .. } => {
            write_link_with_text_writer(file, |f| write_nodes(f, text.iter()), target)
        }
        UnorderedList { items, .. } => {
            let result = write_nodes_with_affix(
                file,
                items.iter().flat_map(|li| li.nodes.iter()),
                |f| write!(f, "- "),
                |f| writeln!(f),
            );
            writeln!(file)?;
            result
        }
        Redirect { target, .. } => write_link(file, target, target),
        Text { value, .. } => {
            write!(file, "{}", value)?;
            Ok(true)
        }

        Bold { .. } => Ok(false),
        BoldItalic { .. } => Ok(false),
        Category {
            ordinal, target, ..
        } => Ok(false),
        CharacterEntity { character, .. } => Ok(false),
        Comment { .. } => Ok(false),
        DefinitionList { items, .. } => Ok(false),
        EndTag { name, .. } => Ok(false),
        ExternalLink { nodes, .. } => Ok(false),
        HorizontalDivider { .. } => Ok(false),
        Image { target, text, .. } => Ok(false),
        Italic { .. } => Ok(false),
        MagicWord { .. } => Ok(false),
        OrderedList { items, .. } => Ok(false),
        ParagraphBreak { .. } => Ok(false),
        Parameter { default, name, .. } => Ok(false),
        Preformatted { nodes, .. } => Ok(false),
        Table {
            attributes,
            captions,
            rows,
            ..
        } => Ok(false),
        Tag { name, nodes, .. } => Ok(false),
        Template {
            name, parameters, ..
        } => Ok(false),
        StartTag { name, .. } => Ok(false),
    }
}

fn write_link_with_text_writer(
    file: &mut File,
    mut text_writer: impl FnMut(&mut File) -> anyhow::Result<bool>,
    target: &str,
) -> anyhow::Result<bool> {
    write!(file, "[")?;
    let result = text_writer(file);
    write!(file, "]({})", resolve_link(target))?;
    result
}

fn write_link(file: &mut File, text: &str, target: &str) -> anyhow::Result<bool> {
    write_link_with_text_writer(
        file,
        |f| {
            write!(f, "{}", text)?;
            Ok(true)
        },
        target,
    )
}

fn write_nodes_with_affix<'a>(
    file: &mut File,
    nodes: impl Iterator<Item = &'a parse_wiki_text::Node<'a>>,
    mut prefixer: impl FnMut(&mut File) -> std::io::Result<()>,
    mut postfixer: impl FnMut(&mut File) -> std::io::Result<()>,
) -> anyhow::Result<bool> {
    for node in nodes {
        prefixer(file)?;
        let written = write_node(file, node)?;
        postfixer(file)?;

        if !written {
            return Ok(false);
        }
    }
    Ok(true)
}

fn write_nodes<'a>(
    file: &mut File,
    nodes: impl Iterator<Item = &'a parse_wiki_text::Node<'a>>,
) -> anyhow::Result<bool> {
    write_nodes_with_affix(file, nodes, |_| Ok(()), |_| Ok(()))
}

fn resolve_link(link: &str) -> String {
    link.replace(" ", "_")
}
