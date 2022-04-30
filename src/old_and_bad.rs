use std::{fs::File, io::Write};

pub(crate) fn write_node(file: &mut File, node: &parse_wiki_text::Node) -> anyhow::Result<bool> {
    use parse_wiki_text::Node::*;

    #[allow(unused_variables)]
    match node {
        Heading { level, nodes, .. } => {
            write!(file, "{} ", "#".repeat(*level as usize))?;
            let result = write_nodes(file, nodes.iter());
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

pub(crate) fn write_link_with_text_writer(
    file: &mut File,
    mut text_writer: impl FnMut(&mut File) -> anyhow::Result<bool>,
    target: &str,
) -> anyhow::Result<bool> {
    write!(file, "[")?;
    let result = text_writer(file);
    write!(file, "]({})", resolve_link(target))?;
    result
}

pub(crate) fn write_link(file: &mut File, text: &str, target: &str) -> anyhow::Result<bool> {
    write_link_with_text_writer(
        file,
        |f| {
            write!(f, "{}", text)?;
            Ok(true)
        },
        target,
    )
}

pub(crate) fn write_nodes_with_affix<'a>(
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

pub(crate) fn write_nodes<'a>(
    file: &mut File,
    nodes: impl Iterator<Item = &'a parse_wiki_text::Node<'a>>,
) -> anyhow::Result<bool> {
    write_nodes_with_affix(file, nodes, |_| Ok(()), |_| Ok(()))
}

pub(crate) fn resolve_link(link: &str) -> String {
    link.replace(" ", "_")
}
