//! Pure matching rules shared by listing streams.
use crate::directory::ListFilter;
use crate::directory::ListOptions;
use crate::metadata::DirEntry;
use crate::path::Path;
use crate::path::PathSemantics;
pub(crate) fn relative_path<'a>(root: &Path, entry: &'a Path, semantics: PathSemantics) -> Option<&'a str> {
    if matches!(semantics, PathSemantics::ObjectKey | PathSemantics::ProviderSpecific) {
        return entry.as_str().strip_prefix(root.as_str());
    }
    if root == entry {
        Some("")
    } else if root.as_str() == "/" {
        entry.as_str().strip_prefix('/')
    } else {
        let remainder = entry.as_str().strip_prefix(root.as_str())?;
        if root.as_str().ends_with('/') {
            Some(remainder)
        } else {
            remainder.strip_prefix('/')
        }
    }
}
pub(crate) fn select(
    entry: &DirEntry,
    root: &Path,
    options: &ListOptions,
    semantics: PathSemantics,
) -> Result<(), &'static str> {
    let Some(relative) = relative_path(root, &entry.path, semantics) else {
        return Err("provider returned directory entry outside requested root");
    };
    match (semantics, options.filter()) {
        (PathSemantics::Hierarchical, Some(ListFilter::Subtree(prefix)))
            if !(relative == prefix || relative.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('/'))) =>
        {
            return Err("provider returned directory entry outside requested prefix");
        }
        (PathSemantics::Hierarchical, Some(ListFilter::LiteralPrefix(_))) => {
            return Err("literal prefix is not valid for hierarchical listing");
        }
        (PathSemantics::ObjectKey | PathSemantics::ProviderSpecific, Some(ListFilter::LiteralPrefix(prefix)))
            if !relative.starts_with(prefix) =>
        {
            return Err("provider returned directory entry outside requested prefix");
        }
        (PathSemantics::ObjectKey | PathSemantics::ProviderSpecific, Some(ListFilter::Subtree(_))) => {
            return Err("subtree filter is not valid for flat listing");
        }
        _ => {}
    }
    if !options.recursive() && options.filter().is_none() && relative.contains('/') {
        return Err("provider returned nested directory entry for non-recursive listing");
    }
    if options.include_metadata() && entry.metadata.is_none() {
        return Err("provider returned directory entry without requested metadata");
    }
    Ok(())
}
