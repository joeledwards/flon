//! Hand-reviewed ports of node-flon/test/unmarshal.js plus Rust extensions.
use super::*;

struct Fixture {
    name: &'static str,
    input: String,
    expected: String,
}

fn fixture(name: &'static str, input: &str, expected: &str) -> Fixture {
    Fixture {
        name,
        input: input.into(),
        expected: expected.into(),
    }
}

fn object_fixture(name: &'static str, members: &str, assignments: &str) -> Fixture {
    fixture(
        name,
        &format!("{{{members}}}"),
        &format!("json = {{}};\n{assignments}"),
    )
}

fn node_fixtures() -> Vec<Fixture> {
    let mut fixtures = vec![fixture("empty object", "{}", "json = {};\n")];
    for (name, members, expected) in [
        ("string", r#""k":"v""#, "json.k = \"v\";\n"),
        ("null", r#""n":null"#, "json.n = null;\n"),
        ("true", r#""b":true"#, "json.b = true;\n"),
        ("false", r#""b":false"#, "json.b = false;\n"),
        ("zero", r#""n":0"#, "json.n = 0;\n"),
        ("integer", r#""n":1"#, "json.n = 1;\n"),
        ("fraction", r#""n":1.1"#, "json.n = 1.1;\n"),
        ("small fraction", r#""n":0.1"#, "json.n = 0.1;\n"),
        ("text", r#""t":"foobar""#, "json.t = \"foobar\";\n"),
        (
            "array member",
            r#""a":[1]"#,
            "json.a = [];\njson.a[0] = 1;\n",
        ),
        (
            "hyphenated key",
            r#""hyphen-ate":0"#,
            "json[\"hyphen-ate\"] = 0;\n",
        ),
        ("quoted key", r#""\"q\"":1"#, "json[\"\\\"q\\\"\"] = 1;\n"),
    ] {
        fixtures.push(object_fixture(name, members, expected));
    }
    fixtures.extend([
        fixture("string array", r#"["a"]"#, "json = [];\njson[0] = \"a\";\n"),
        fixture("mixed scalar array", r#"[1,"2"]"#, "json = [];\njson[0] = 1;\njson[1] = \"2\";\n"),
        object_fixture("ordered record objects",
            r#""records":[{"id":1,"name":"first"},{"name":"second","id":2},{"id":3,"name":"third"}]"#,
            "json.records = [];\njson.records[0] = {};\njson.records[0].id = 1;\njson.records[0].name = \"first\";\njson.records[1] = {};\njson.records[1].name = \"second\";\njson.records[1].id = 2;\njson.records[2] = {};\njson.records[2].id = 3;\njson.records[2].name = \"third\";\n"),
        object_fixture("nested array indexes", r#""records":[[0,1],[2,3],[4,5],[6,7]]"#,
            "json.records = [];\njson.records[0] = [];\njson.records[0][0] = 0;\njson.records[0][1] = 1;\njson.records[1] = [];\njson.records[1][0] = 2;\njson.records[1][1] = 3;\njson.records[2] = [];\njson.records[2][0] = 4;\njson.records[2][1] = 5;\njson.records[3] = [];\njson.records[3][0] = 6;\njson.records[3][1] = 7;\n"),
        fixture("consecutive empty objects", "{}\n{}\n", "json = {};\n"),
        fixture("empty then populated object", "{}\n{\"id\":\"a\"}\n", "json = {};\njson.id = \"a\";\n"),
        fixture("consecutive updates", "{\"id\":\"a\",\"value\":1}\n{\"id\":\"b\",\"value\":2}",
            "json = {};\njson.id = \"a\";\njson.value = 1;\njson.id = \"b\";\njson.value = 2;\n"),
    ]);
    fixtures
}

fn extension_fixtures() -> Vec<Fixture> {
    vec![
        fixture("empty root array", "[]", "json = [];\n"),
        object_fixture(
            "empty nested containers",
            r#""object":{},"array":[]"#,
            "json.object = {};\njson.array = [];\n",
        ),
        object_fixture(
            "duplicate escaped and literal keys",
            r#""a":1,"\u0061":2,"a":3"#,
            "json.a = 1;\njson[\"\\u0061\"] = 2;\njson.a = 3;\n",
        ),
        object_fixture(
            "Unicode and numeric keys",
            r#""雪":"😀","0":0,"":false"#,
            "json[\"雪\"] = \"😀\";\njson[\"0\"] = 0;\njson[\"\"] = false;\n",
        ),
        object_fixture(
            "backslashes and controls",
            r#""a\\b":"\b\f\n\r\t\u0000\"\\\/""#,
            "json[\"a\\\\b\"] = \"\\b\\f\\n\\r\\t\\u0000\\\"\\\\\\/\";\n",
        ),
        fixture(
            "precise numbers",
            "[9007199254740993,-0,1.2300,1e+009,1E-999]",
            "json = [];\njson[0] = 9007199254740993;\njson[1] = -0;\njson[2] = 1.2300;\njson[3] = 1e+009;\njson[4] = 1E-999;\n",
        ),
        fixture(
            "adjacent mixed roots",
            "{}[1]{\"x\":2}[3]",
            "json = {};\njson[0] = 1;\njson.x = 2;\njson[0] = 3;\n",
        ),
        fixture(
            "junk between candidates",
            "log {\"a\":1} junk [true] tail",
            "json = {};\njson.a = 1;\njson[0] = true;\n",
        ),
    ]
}

fn assert_fixture(fixture: Fixture) {
    let input = fixture.input.as_bytes();
    // Check the convenience API as well as the public event-consumer path.
    assert_eq!(
        format(input, Vec::new()).unwrap(),
        fixture.expected.as_bytes(),
        "{}",
        fixture.name
    );
    for mode in [
        OutputMode::Live,
        OutputMode::Confirmed(ConfirmedOutput {
            memory_threshold: 4096,
            ..ConfirmedOutput::default()
        }),
    ] {
        for cuts in std::iter::once((0..=input.len()).collect::<Vec<_>>())
            .chain((0..=input.len()).map(|split| vec![split]))
        {
            let mut parser = Parser::default();
            let mut formatter = Formatter::with_config(
                Vec::new(),
                Config {
                    output_mode: mode.clone(),
                    ..Config::default()
                },
            );
            let mut start = 0;
            for &end in &cuts {
                parser.push(&input[start..end], |event| formatter.event(event).unwrap());
                start = end;
            }
            parser.push(&input[start..], |event| formatter.event(event).unwrap());
            parser.finish(|event| formatter.event(event).unwrap());
            assert_eq!(
                formatter.into_inner(),
                fixture.expected.as_bytes(),
                "{}, mode {mode:?}, cuts {cuts:?}",
                fixture.name
            );
        }
    }
}

#[test]
fn active_node_fixtures_preserve_plain_output() {
    for fixture in node_fixtures() {
        assert_fixture(fixture);
    }
}

#[test]
fn rust_extensions_preserve_escaping_precision_and_stream_order() {
    for fixture in extension_fixtures() {
        assert_fixture(fixture);
    }
}

#[test]
fn legacy_junk_fixture_preserves_all_array_elements() {
    // The disabled Node expectation omitted the third string and shifted the
    // fourth element to index 2. Preserve every element in encounter order.
    assert_fixture(fixture(
        "legacy junk fixture with corrected indexes",
        r#"
        {
          "name": "willy",
          "meta": [
            {"key": "thing", "value": "is cool"},
            {"key": "other", "value": "is also cool"}
          ]
        }
        junk
        {
          "disposition": "free",
          "turtles": {"turtles": {"turtles": {"turtles": [
            "turtles", "turtles", "turtles",
            {"turtles": {"turtles": "turtles"}}
          ]}}}
        }
        more junk
        {"name": "leroy"}
        "#,
        concat!(
            "json = {};\n",
            "json.name = \"willy\";\n",
            "json.meta = [];\n",
            "json.meta[0] = {};\n",
            "json.meta[0].key = \"thing\";\n",
            "json.meta[0].value = \"is cool\";\n",
            "json.meta[1] = {};\n",
            "json.meta[1].key = \"other\";\n",
            "json.meta[1].value = \"is also cool\";\n",
            "json.disposition = \"free\";\n",
            "json.turtles = {};\n",
            "json.turtles.turtles = {};\n",
            "json.turtles.turtles.turtles = {};\n",
            "json.turtles.turtles.turtles.turtles = [];\n",
            "json.turtles.turtles.turtles.turtles[0] = \"turtles\";\n",
            "json.turtles.turtles.turtles.turtles[1] = \"turtles\";\n",
            "json.turtles.turtles.turtles.turtles[2] = \"turtles\";\n",
            "json.turtles.turtles.turtles.turtles[3] = {};\n",
            "json.turtles.turtles.turtles.turtles[3].turtles = {};\n",
            "json.turtles.turtles.turtles.turtles[3].turtles.turtles = \"turtles\";\n",
            "json.name = \"leroy\";\n",
        ),
    ));
}
