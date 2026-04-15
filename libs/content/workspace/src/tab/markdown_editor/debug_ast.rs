use colored::Colorize as _;
use comrak::nodes::AstNode;

pub fn print_ast<'a>(root: &'a AstNode<'a>) {
    print_recursive(root, "");
}

fn print_recursive<'a>(node: &'a AstNode<'a>, indent: &str) {
    let last_child = node.next_sibling().is_none();
    let sourcepos = node.data.borrow().sourcepos;

    if indent.is_empty() {
        println!(
            "{} {:?} {}{}{}",
            if node.data.borrow().value.block() { "□" } else { "☰" }.blue(),
            node.data.borrow().value,
            format!("{sourcepos}").yellow(),
            if node.children().count() > 0 {
                format!(" +{} ", node.children().count())
            } else {
                "".into()
            }
            .blue(),
            if node.children().count() > 0 {
                if !node.data.borrow().value.block() || node.data.borrow().value.contains_inlines()
                {
                    "☰"
                } else {
                    "□"
                }
            } else {
                ""
            }
            .bright_magenta(),
        );
    } else {
        println!(
            "{}{}{} {:?} {}{}{}",
            indent,
            if last_child { "└>" } else { "├>" }.bright_black(),
            if node.data.borrow().value.block() { "□" } else { "☰" }.blue(),
            node.data.borrow().value,
            format!("{sourcepos}").yellow(),
            if node.children().count() > 0 {
                format!(" +{} ", node.children().count())
            } else {
                "".into()
            }
            .blue(),
            if node.children().count() > 0 {
                if !node.data.borrow().value.block() || node.data.borrow().value.contains_inlines()
                {
                    "☰"
                } else {
                    "□"
                }
            } else {
                ""
            }
            .bright_magenta(),
        );
    }
    for child in node.children() {
        print_recursive(
            child,
            &format!("{}{}", indent, if last_child { "  " } else { "│ " }.bright_black()),
        );
    }
}
