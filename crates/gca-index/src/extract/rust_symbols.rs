use gca_core::{RustSymbolDescriptor, RustSymbolKind};
use std::fs;
use std::path::Path;
use syn::{Attribute, Fields, ImplItem, Item, ItemImpl, UseTree, Visibility};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RustSymbolScan {
    pub modules: Vec<String>,
    pub public_items: Vec<String>,
    pub test_targets: Vec<String>,
    pub symbols: Vec<RustSymbolDescriptor>,
}

pub fn extract_rust_symbols(repo_root: &Path) -> RustSymbolScan {
    let mut scan = RustSymbolScan::default();
    gather_rust_files(repo_root, repo_root, &mut scan);
    scan.modules.sort();
    scan.modules.dedup();
    scan.public_items.sort();
    scan.public_items.dedup();
    scan.test_targets.sort();
    scan.test_targets.dedup();
    scan.symbols.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.name.cmp(&right.name))
            .then(symbol_kind_label(left.kind).cmp(symbol_kind_label(right.kind)))
    });
    scan.symbols.dedup();
    scan
}

fn gather_rust_files(root: &Path, current: &Path, scan: &mut RustSymbolScan) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if should_skip_dir(&name) {
            continue;
        }

        if path.is_dir() {
            gather_rust_files(root, &path, scan);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative = relative.display().to_string();
        scan.modules.push(relative.clone());
        if relative.contains("/tests/") || relative.starts_with("tests/") {
            scan.test_targets.push(relative.clone());
        }

        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = syn::parse_file(&raw) else {
            continue;
        };
        collect_items(&parsed.items, &raw, &relative, &mut Vec::new(), scan);
    }
}

fn collect_items(
    items: &[Item],
    raw: &str,
    file_path: &str,
    module_path: &mut Vec<String>,
    scan: &mut RustSymbolScan,
) {
    for item in items {
        match item {
            Item::Fn(function) => {
                let is_test = is_test(&function.attrs);
                if is_visible(&function.vis) || is_test {
                    let name = qualified_name(module_path, &function.sig.ident.to_string());
                    let visibility = visibility_label(&function.vis);
                    if is_test {
                        scan.test_targets.push(format!("{file_path}::{name}"));
                    }
                    let kind = if is_test {
                        RustSymbolKind::Test
                    } else {
                        RustSymbolKind::Function
                    };
                    let line = find_line(raw, &format!("fn {}", function.sig.ident));
                    push_symbol(scan, name.clone(), kind, visibility, file_path, line);
                    if is_visible(&function.vis) {
                        scan.public_items
                            .push(format!("{} fn {name}", visibility_label(&function.vis)));
                    }
                }
            }
            Item::Struct(item_struct) if is_visible(&item_struct.vis) => {
                let name = qualified_name(module_path, &item_struct.ident.to_string());
                let visibility = visibility_label(&item_struct.vis);
                let line = find_line(raw, &format!("struct {}", item_struct.ident));
                push_symbol(
                    scan,
                    name.clone(),
                    RustSymbolKind::Struct,
                    visibility.clone(),
                    file_path,
                    line,
                );
                scan.public_items
                    .push(format!("{visibility} struct {name}"));
                collect_visible_fields(
                    &item_struct.fields,
                    raw,
                    file_path,
                    module_path,
                    &name,
                    scan,
                );
            }
            Item::Enum(item_enum) if is_visible(&item_enum.vis) => {
                let name = qualified_name(module_path, &item_enum.ident.to_string());
                let visibility = visibility_label(&item_enum.vis);
                let line = find_line(raw, &format!("enum {}", item_enum.ident));
                push_symbol(
                    scan,
                    name.clone(),
                    RustSymbolKind::Enum,
                    visibility.clone(),
                    file_path,
                    line,
                );
                scan.public_items.push(format!("{visibility} enum {name}"));
            }
            Item::Trait(item_trait) if is_visible(&item_trait.vis) => {
                let name = qualified_name(module_path, &item_trait.ident.to_string());
                let visibility = visibility_label(&item_trait.vis);
                let line = find_line(raw, &format!("trait {}", item_trait.ident));
                push_symbol(
                    scan,
                    name.clone(),
                    RustSymbolKind::Trait,
                    visibility.clone(),
                    file_path,
                    line,
                );
                scan.public_items.push(format!("{visibility} trait {name}"));
            }
            Item::Mod(item_mod) if is_visible(&item_mod.vis) => {
                let name = qualified_name(module_path, &item_mod.ident.to_string());
                let visibility = visibility_label(&item_mod.vis);
                let line = find_line(raw, &format!("mod {}", item_mod.ident));
                push_symbol(
                    scan,
                    name.clone(),
                    RustSymbolKind::Module,
                    visibility.clone(),
                    file_path,
                    line,
                );
                scan.public_items.push(format!("{visibility} mod {name}"));
                if let Some((_, nested)) = &item_mod.content {
                    module_path.push(item_mod.ident.to_string());
                    collect_items(nested, raw, file_path, module_path, scan);
                    module_path.pop();
                }
            }
            Item::Use(item_use) if is_visible(&item_use.vis) => {
                let name = use_tree_label(&item_use.tree);
                let visibility = visibility_label(&item_use.vis);
                let line = find_line(raw, "use ");
                push_symbol(
                    scan,
                    name.clone(),
                    RustSymbolKind::Use,
                    visibility.clone(),
                    file_path,
                    line,
                );
                scan.public_items.push(format!("{visibility} use {name}"));
            }
            Item::Impl(item_impl) => collect_impl(item_impl, raw, file_path, module_path, scan),
            _ => {}
        }
    }
}

fn collect_impl(
    item_impl: &ItemImpl,
    raw: &str,
    file_path: &str,
    module_path: &[String],
    scan: &mut RustSymbolScan,
) {
    let Some(type_name) = impl_type_name(item_impl) else {
        return;
    };
    let name = qualified_name(module_path, &format!("impl {type_name}"));
    let line = find_line(raw, &format!("impl {type_name}")).or_else(|| find_line(raw, "impl "));
    push_symbol(
        scan,
        name,
        RustSymbolKind::Impl,
        "inherent".to_string(),
        file_path,
        line,
    );

    for item in &item_impl.items {
        if let ImplItem::Fn(function) = item
            && is_visible(&function.vis)
        {
            let method_name =
                qualified_name(module_path, &format!("{type_name}::{}", function.sig.ident));
            let visibility = visibility_label(&function.vis);
            let line = find_line(raw, &format!("fn {}", function.sig.ident));
            push_symbol(
                scan,
                method_name.clone(),
                RustSymbolKind::Function,
                visibility.clone(),
                file_path,
                line,
            );
            scan.public_items
                .push(format!("{visibility} fn {method_name}"));
        }
    }
}

fn collect_visible_fields(
    fields: &Fields,
    raw: &str,
    file_path: &str,
    module_path: &[String],
    parent_name: &str,
    scan: &mut RustSymbolScan,
) {
    for field in fields {
        if !is_visible(&field.vis) {
            continue;
        }
        let Some(ident) = &field.ident else {
            continue;
        };
        let name = qualified_name(module_path, &format!("{parent_name}::{ident}"));
        let visibility = visibility_label(&field.vis);
        let line = find_line(raw, &ident.to_string());
        push_symbol(
            scan,
            name,
            RustSymbolKind::Struct,
            visibility,
            file_path,
            line,
        );
    }
}

fn push_symbol(
    scan: &mut RustSymbolScan,
    name: String,
    kind: RustSymbolKind,
    visibility: String,
    file_path: &str,
    line: Option<u32>,
) {
    let path = match line {
        Some(line) => format!("{file_path}:{line}"),
        None => file_path.to_string(),
    };
    scan.symbols.push(RustSymbolDescriptor {
        name,
        kind,
        visibility,
        path,
        line,
    });
}

fn is_visible(visibility: &Visibility) -> bool {
    !matches!(visibility, Visibility::Inherited)
}

fn visibility_label(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public(_) => "pub".to_string(),
        Visibility::Restricted(restricted) => {
            let path = restricted
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if path.is_empty() {
                "pub(restricted)".to_string()
            } else {
                format!("pub({path})")
            }
        }
        Visibility::Inherited => "private".to_string(),
    }
}

fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || attr
                .path()
                .segments
                .last()
                .map(|segment| segment.ident == "test")
                .unwrap_or(false)
    })
}

fn impl_type_name(item_impl: &ItemImpl) -> Option<String> {
    match item_impl.self_ty.as_ref() {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn use_tree_label(tree: &UseTree) -> String {
    match tree {
        UseTree::Path(path) => format!("{}::{}", path.ident, use_tree_label(&path.tree)),
        UseTree::Name(name) => name.ident.to_string(),
        UseTree::Rename(rename) => format!("{} as {}", rename.ident, rename.rename),
        UseTree::Glob(_) => "*".to_string(),
        UseTree::Group(group) => group
            .items
            .iter()
            .map(use_tree_label)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn qualified_name(module_path: &[String], name: &str) -> String {
    if module_path.is_empty() {
        name.to_string()
    } else {
        format!("{}::{name}", module_path.join("::"))
    }
}

fn find_line(raw: &str, needle: &str) -> Option<u32> {
    raw.lines()
        .position(|line| line.contains(needle))
        .map(|index| index as u32 + 1)
}

fn symbol_kind_label(kind: RustSymbolKind) -> &'static str {
    match kind {
        RustSymbolKind::Function => "function",
        RustSymbolKind::Struct => "struct",
        RustSymbolKind::Enum => "enum",
        RustSymbolKind::Trait => "trait",
        RustSymbolKind::Impl => "impl",
        RustSymbolKind::Module => "module",
        RustSymbolKind::Use => "use",
        RustSymbolKind::Test => "test",
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | ".greentic-agent" | "node_modules" | ".cargo"
    )
}
