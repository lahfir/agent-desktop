use super::*;

#[test]
fn rejects_unknown_direction() {
    assert_eq!(
        parse_direction("sideways").unwrap_err().code(),
        "INVALID_ARGS"
    );
}

#[test]
fn rejects_unknown_get_property() {
    match parse_get_property("placeholder") {
        Ok(_) => panic!("expected invalid get property"),
        Err(err) => assert_eq!(err.code(), "INVALID_ARGS"),
    }
}

#[test]
fn rejects_unknown_is_property() {
    match parse_is_property("selected") {
        Ok(_) => panic!("expected invalid is property"),
        Err(err) => assert_eq!(err.code(), "INVALID_ARGS"),
    }
}

#[test]
fn rejects_unknown_mouse_button() {
    assert_eq!(
        parse_mouse_button("primary").unwrap_err().code(),
        "INVALID_ARGS"
    );
}

#[test]
fn parses_xy_with_whitespace() {
    assert_eq!(parse_xy(" 10.5, 20 ").unwrap(), (10.5, 20.0));
}

#[test]
fn rejects_bad_xy_shape_and_numbers() {
    assert_eq!(parse_xy("10").unwrap_err().code(), "INVALID_ARGS");
    assert_eq!(parse_xy("x,20").unwrap_err().code(), "INVALID_ARGS");
    assert_eq!(parse_xy("10,y").unwrap_err().code(), "INVALID_ARGS");
}
