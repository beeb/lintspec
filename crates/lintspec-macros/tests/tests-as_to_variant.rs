use lintspec_macros::AsToVariant;

#[derive(AsToVariant)]
enum TestEnum {
    String(String),
    Int(i32),
    Bool(bool),
}

#[derive(AsToVariant)]
enum CamelCaseEnum {
    FooBar(String),
    HTTPServer(i32),
    XMLParser(bool),
    URLPath(String),
    IOError(String),
}

#[test]
fn test_as_variant_match() {
    let string = TestEnum::String("test".to_string());
    assert_eq!(string.as_string().unwrap(), "test");

    let int = TestEnum::Int(42);
    assert_eq!(*int.as_int().unwrap(), 42);

    let bool = TestEnum::Bool(true);
    assert!(bool.as_bool().unwrap());
}

#[test]
fn test_as_variant_nomatch() {
    let string = TestEnum::String("test".to_string());
    assert!(string.as_int().is_none());
    assert!(string.as_bool().is_none());

    let int = TestEnum::Int(42);
    assert!(int.as_string().is_none());
    assert!(int.as_bool().is_none());
}

#[test]
fn test_to_variant_match() {
    assert_eq!(
        TestEnum::String("test".to_string()).to_string().unwrap(),
        "test"
    );
    assert_eq!(TestEnum::Int(42).to_int().unwrap(), 42);
    assert!(TestEnum::Bool(true).to_bool().unwrap());
}

#[test]
fn test_to_variant_nomatch() {
    assert!(TestEnum::String("test".to_string()).to_int().is_none());
    assert!(TestEnum::String("test".to_string()).to_bool().is_none());

    assert!(TestEnum::Int(42).to_string().is_none());
    assert!(TestEnum::Int(42).to_bool().is_none());
}

#[test]
fn test_case_conversion() {
    let foo_bar = CamelCaseEnum::FooBar("test".to_string());
    assert!(foo_bar.as_foo_bar().is_some());
    assert!(foo_bar.to_foo_bar().is_some());

    let http_server = CamelCaseEnum::HTTPServer(8080);
    assert!(http_server.as_http_server().is_some());
    assert!(http_server.to_http_server().is_some());

    let xml_parser = CamelCaseEnum::XMLParser(true);
    assert!(xml_parser.as_xml_parser().is_some());
    assert!(xml_parser.to_xml_parser().is_some());

    let url_path = CamelCaseEnum::URLPath("/home".to_string());
    assert!(url_path.as_url_path().is_some());
    assert!(url_path.to_url_path().is_some());

    let io_error = CamelCaseEnum::IOError("file not found".to_string());
    assert!(io_error.as_io_error().is_some());
    assert!(io_error.to_io_error().is_some());
}
