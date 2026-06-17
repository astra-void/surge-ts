#[cfg(test)]
mod tests {
    #[test]
    fn dump() {
        let allocator = oxc_allocator::Allocator::default();
        let source_type = oxc_span::SourceType::default().with_typescript(true);
        let ret = oxc_parser::Parser::new(&allocator, "a?.b?.c[0]", source_type).parse();
        println!("{:#?}", ret.program);
    }
}
