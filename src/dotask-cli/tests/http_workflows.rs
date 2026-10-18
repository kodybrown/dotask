use serde_json::json;
use std::{
  fs,
  io::{Read, Write},
  net::TcpListener,
  path::Path,
  process::Command,
  thread,
  time::{Duration, Instant},
};

fn fixture(responses: Vec<(u16, String, String)>) -> (String, thread::JoinHandle<Vec<String>>) {
  fixture_bytes(
    responses
      .into_iter()
      .map(|(status, headers, body)| (status, headers, body.into_bytes()))
      .collect(),
  )
}

fn fixture_bytes(responses: Vec<(u16, String, Vec<u8>)>) -> (String, thread::JoinHandle<Vec<String>>) {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  listener.set_nonblocking(true).unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let url = base.clone();
  let worker = thread::spawn(move || {
    let mut requests = Vec::new();
    for (status, headers, body) in responses {
      let deadline = Instant::now() + Duration::from_secs(10);
      let mut stream = loop {
        if let Ok((stream, _)) = listener.accept() {
          break stream;
        }
        assert!(Instant::now() < deadline, "HTTP fixture did not receive its expected request.");
        thread::sleep(Duration::from_millis(10));
      };
      stream.set_nonblocking(false).unwrap();
      stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
      let mut bytes = Vec::new();
      let mut buffer = [0; 1024];
      while !bytes.windows(4).any(|v| v == b"\r\n\r\n") {
        let n = stream.read(&mut buffer).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&buffer[..n]);
      }
      requests.push(String::from_utf8(bytes).unwrap());
      let headers = headers.replace("{base}", &url);
      let response = format!("HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n",body.len());
      let _ = stream.write_all(response.as_bytes());
      let _ = stream.write_all(&body);
    }
    requests
  });
  (base, worker)
}
fn setup(root: &Path, task: &str, url: &str) {
  fs::create_dir_all(root.join(".tasks")).unwrap();
  fs::write(root.join(".tasks/refresh.task"), task).unwrap();
  fs::write(
    root.join(".dotasks.yaml"),
    format!("settings:\n  url: {}\n", serde_json::to_string(url).unwrap()),
  )
  .unwrap();
}
fn run(root: &Path, args: &[&str]) -> std::process::Output {
  Command::new(env!("CARGO_BIN_EXE_dotask"))
    .args(args)
    .current_dir(root)
    .env("PATH", "")
    .env("DOTASK_CACHE_HOME", root.join("cache"))
    .output()
    .unwrap()
}

const TASK: &str = r#"
description: Refresh repository documents
steps:
  - id: repositories
    http:
      method: GET
      url: {setting: url}
      query: {type: owner, per_page: 100, search: 'a & b'}
      headers: {Accept: application/vnd.github+json}
      response-format: json
      pagination: {mode: link-header, relation: next}
  - id: originals
    json: {operation: filter, input: {result: repositories.data}, where: {field: fork, equals: false}}
  - id: projected
    json: {operation: select, input: {result: originals.data}, fields: {name: name, id: id, description: description, archived: archived}}
  - foreach: {items: {result: projected.data}, as: repo}
    steps:
      - id: path
        text: {template: 'projects/{{name}}.md', values: {name: {item: repo.name}}}
      - id: document
        file:
          operation: frontmatter
          path: {result: path.text}
          create-if-missing: true
          initial: {title: {item: repo.name}, summary: '', screenshot: '', hidden: false}
          match-existing: {github.id: {item: repo.id}}
          set: {github.id: {item: repo.id}, github.description: {item: repo.description}, github.archived: {item: repo.archived}}
      - run: audit
        with: {name: {item: repo.name}}
"#;

#[test]
fn paginated_refresh_creates_filters_updates_preserves_and_runs_without_sdks() {
  let root = tempfile::tempdir().unwrap();
  let first = json!([{"id":1,"name":"fixeol","description":"first","fork":false,"archived":true},{"id":9,"name":"forked","fork":true}]).to_string();
  let updated = json!([{"id":1,"name":"fixeol","description":null,"fork":false,"archived":false}]).to_string();
  let second =
    json!([{"id":2,"name":"dotask","description":"A task runner.","fork":false,"archived":false}]).to_string();
  let next = "Link: <{base}/repos?page=2>; rel=\"next\"\r\n".to_string();
  let (url, server) = fixture(vec![
    (200, next.clone(), first),
    (200, String::new(), second.clone()),
    (200, next.clone(), updated.clone()),
    (200, String::new(), second.clone()),
    (200, next, updated),
    (200, String::new(), second),
  ]);
  setup(root.path(), TASK, &format!("{url}/repos"));
  fs::write(root.path().join(".tasks/audit.task"),"options: [{name: name, required: true}]\nsteps:\n  - id: path\n    text: {template: 'audit/{{name}}', values: {name: {parameter: name}}}\n  - file: {operation: write, path: {result: path.text}, content: {parameter: name}}\n").unwrap();
  let output = run(root.path(), &["refresh"]);
  assert!(output.status.success(), "{output:?}");
  assert!(!root.path().join("projects/forked.md").exists());
  let path = root.path().join("projects/fixeol.md");
  let created = fs::read_to_string(&path).unwrap();
  let authored = created
    .replace("summary: \"\"", "summary: 'My summary' # keep")
    .replace("screenshot: \"\"", "screenshot: /assets/fixeol.png")
    + "\n# Why I built this\n\nHandwritten notes.\n![Screenshot](/image.png)";
  fs::write(&path, &authored).unwrap();
  let output = run(root.path(), &["refresh"]);
  assert!(output.status.success(), "{output:?}");
  let refreshed = fs::read_to_string(&path).unwrap();
  assert!(refreshed.contains("summary: 'My summary' # keep"));
  assert!(refreshed.contains("screenshot: /assets/fixeol.png"));
  assert!(refreshed.contains("description: null"));
  assert!(refreshed.contains("archived: false"));
  assert!(refreshed.ends_with("Handwritten notes.\n![Screenshot](/image.png)"));
  let modified = fs::metadata(&path).unwrap().modified().unwrap();
  let output = run(root.path(), &["refresh"]);
  assert!(output.status.success(), "{output:?}");
  assert_eq!(fs::read_to_string(&path).unwrap(), refreshed);
  assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
  assert_eq!(fs::read_to_string(root.path().join("audit/dotask")).unwrap(), "dotask");
  let requests = server.join().unwrap();
  assert!(requests[0].contains("search=a%20%26%20b"), "{}", requests[0]);
  assert!(requests[1].starts_with("GET /repos?page=2 "));
}

#[test]
fn http_errors_never_publish_partial_pages_or_execute_later_steps() {
  for responses in [
    vec![(404, String::new(), "{}".into())],
    vec![(200, String::new(), "invalid JSON".into())],
    vec![(302, "Location: http://example.invalid\r\n".into(), "{}".into())],
    vec![(
      200,
      "Link: <https://example.invalid/repos>; rel=\"next\"\r\n".into(),
      "[]".into(),
    )],
    vec![(200, "Link: <{base}/repos>; rel=\"next\"\r\n".into(), "[]".into())],
    vec![
      (200, "Link: <{base}/next>; rel=\"next\"\r\n".into(), "[]".into()),
      (500, String::new(), "{}".into()),
    ],
  ] {
    let root = tempfile::tempdir().unwrap();
    let (url, server) = fixture(responses);
    setup(root.path(),"steps:\n  - id: repos\n    http: {method: GET, url: {setting: url}, response-format: json, pagination: {mode: link-header, relation: next}}\n  - file: {operation: write, path: wrong, content: wrong}\n",&format!("{url}/repos"));
    let output = run(root.path(), &["refresh"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(!root.path().join("wrong").exists());
    server.join().unwrap();
  }
}

#[test]
fn help_validates_nested_scopes_without_requests_and_loops_do_not_leak_results() {
  let root = tempfile::tempdir().unwrap();
  setup(root.path(), TASK, "http://127.0.0.1:1/not-contacted");
  let output = run(root.path(), &["help", "refresh"]);
  assert!(output.status.success(), "{output:?}");
  assert!(String::from_utf8_lossy(&output.stdout).contains("foreach"));
  for invalid in [
    "steps: [{text: {template: '{{x}}', values: {x: {item: repo.name}}}}]",
    "steps: [{foreach: {items: [], as: repo}, steps: [{id: inner, text: {template: x, values: {}}}]}, {file: {operation: write, path: wrong, content: {result: inner.text}}}]",
    "steps: [{foreach: {items: [], as: repo}, steps: [{file: {operation: unknown, path: wrong}}]}]",
  ] {
    fs::write(root.path().join(".tasks/refresh.task"),invalid).unwrap();
    assert!(!run(root.path(),&["refresh"]).status.success());
  }
  setup(root.path(),"steps:\n  - foreach: {items: [{enabled: true}, {enabled: false}], as: repo}\n    steps:\n      - id: condition\n        json: {operation: select, input: {item: repo}, fields: {enabled: enabled}}\n      - id: maybe\n        text: {template: x, values: {}}\n        when: {result: condition.data.enabled}\n      - file: {operation: write, path: first, content: {result: maybe.text}}\n","");
  let output = run(root.path(), &["refresh"]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("unavailable"), "{output:?}");
}

#[test]
fn frontmatter_file_writes_preserve_encodings_and_refuse_wrong_identity() {
  let root = tempfile::tempdir().unwrap();
  let original =
    "---\r\nsummary: 'Handwritten' # keep\r\ngithub:\r\n  id: 1\r\n  language: old\r\n---\r\n\r\nNotes \u{1f680}";
  let path = root.path().join("page.md");
  for encoding in 0..3 {
    let bytes = match encoding {
      0 => [vec![0xef, 0xbb, 0xbf], original.as_bytes().to_vec()].concat(),
      1 => [
        vec![0xff, 0xfe],
        original.encode_utf16().flat_map(u16::to_le_bytes).collect(),
      ]
      .concat(),
      _ => [
        vec![0xfe, 0xff],
        original.encode_utf16().flat_map(u16::to_be_bytes).collect(),
      ]
      .concat(),
    };
    fs::write(&path, &bytes).unwrap();
    setup(root.path(),"steps: [{file: {operation: frontmatter, path: page.md, match-existing: {github.id: 2}, set: {github.language: Rust}}}]","");
    assert!(!run(root.path(), &["refresh"]).status.success());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    setup(root.path(),"steps: [{file: {operation: frontmatter, path: page.md, match-existing: {github.id: 1}, set: {github.language: Rust}}}]","");
    let output = run(root.path(), &["refresh"]);
    assert!(output.status.success(), "{output:?}");
    let actual = fs::read(&path).unwrap();
    let expected = original.replace("language: old", "language: \"Rust\"");
    let expected = match encoding {
      0 => [vec![0xef, 0xbb, 0xbf], expected.into_bytes()].concat(),
      1 => [
        vec![0xff, 0xfe],
        expected.encode_utf16().flat_map(u16::to_le_bytes).collect(),
      ]
      .concat(),
      _ => [
        vec![0xfe, 0xff],
        expected.encode_utf16().flat_map(u16::to_be_bytes).collect(),
      ]
      .concat(),
    };
    assert_eq!(actual, expected);
  }
}

#[test]
fn documented_github_task_executes_against_an_explicit_local_endpoint() {
  let root = tempfile::tempdir().unwrap();
  let body = json!([{"id":123456,"name":"fixeol","full_name":"kodybrown/fixeol","html_url":"https://github.com/kodybrown/fixeol","description":"Fixes line endings.","pushed_at":"2026-10-13T23:17:05Z","language":"Rust","archived":false,"fork":false}]).to_string();
  let (url, server) = fixture(vec![(200, String::new(), body)]);
  setup(
    root.path(),
    include_str!("../../../examples/github-projects/.tasks/refresh-projects.task"),
    "",
  );
  let output = run(root.path(), &["refresh", "--api-url", &url]);
  assert!(output.status.success(), "{output:?}");
  let text = fs::read_to_string(root.path().join("src/projects/fixeol.md")).unwrap();
  assert!(text.contains("repository: \"kodybrown/fixeol\""), "{text}");
  assert!(text.contains("pushedAt: \"2026-10-13T23:17:05Z\""));
  assert!(text.contains("summary: \"\""));
  server.join().unwrap();
}

#[test]
fn http_limits_and_timeout_fail_before_mutations() {
  for (headers, body, extra) in [
    (
      "Link: <{base}/second>; rel=\"next\"\r\n".to_string(),
      "[]".to_string(),
      ", pagination: {mode: link-header, relation: next, max-pages: 1}",
    ),
    (String::new(), format!("\"{}\"", "x".repeat(16 * 1024 * 1024)), ""),
  ] {
    let root = tempfile::tempdir().unwrap();
    let (url, server) = fixture(vec![(200, headers, body)]);
    setup(root.path(),&format!("steps:\n  - http: {{method: GET, url: {{setting: url}}, response-format: json{extra}}}\n  - file: {{operation: write, path: wrong, content: wrong}}\n"),&url);
    let output = run(root.path(), &["refresh"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(!root.path().join("wrong").exists());
    server.join().unwrap();
  }
  let root = tempfile::tempdir().unwrap();
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let url = format!("http://{}", listener.local_addr().unwrap());
  listener.set_nonblocking(true).unwrap();
  // A live listener with no response exercises the whole-action deadline.
  setup(root.path(),"steps:\n  - http: {method: GET, url: {setting: url}, response-format: json, timeout-seconds: 1}\n  - file: {operation: write, path: wrong, content: wrong}\n",&url);
  let started = Instant::now();
  let output = run(root.path(), &["refresh"]);
  assert!(!output.status.success());
  assert!(started.elapsed() < Duration::from_secs(5));
  assert!(String::from_utf8_lossy(&output.stderr).contains("timed out"), "{output:?}");
  assert!(!root.path().join("wrong").exists());
}

#[test]
fn referenced_objects_are_data_not_reference_programs() {
  let root = tempfile::tempdir().unwrap();
  let (url, server) = fixture(vec![(200, String::new(), "[{\"setting\":\"nonexistent\"}]".into())]);
  setup(root.path(),"steps:\n  - id: remote\n    http: {method: GET, url: {setting: url}, response-format: json}\n  - foreach: {items: {result: remote.data}, as: object}\n    steps:\n      - id: projected\n        json: {operation: select, input: {item: object}, fields: {text: setting}}\n      - file: {operation: write, path: preserved, content: {result: projected.data.text}}\n",&url);
  let output = run(root.path(), &["refresh"]);
  assert!(output.status.success(), "{output:?}");
  assert_eq!(fs::read_to_string(root.path().join("preserved")).unwrap(), "nonexistent");
  server.join().unwrap();
}

#[test]
fn named_containers_and_nested_loops_preserve_outer_scope() {
  let root = tempfile::tempdir().unwrap();
  setup(
    root.path(),
    r#"
steps:
  - id: outer
    text: {template: '{{result}} {{item}} {{parameter}}', values: {result: literal, item: names, parameter: work}}
  - foreach: {items: [{name: first}, {name: second}], as: repo}
    steps:
      - foreach: {items: [1, 2], as: index}
        steps:
          - id: inner
            text: {template: '{{repo}}-{{index}}', values: {repo: {item: repo.name}, index: {item: index}}}
  - file: {operation: write, path: outer, content: {result: outer.text}}
  - id: projected
    json: {operation: select, input: {a: selected}, fields: {result: a}}
  - file: {operation: write, path: selected, content: {result: projected.data.result}}
  - file: {operation: frontmatter, path: page.md, create-if-missing: true, set: {result: literal}}
  - run: child
    with: {item: {result: outer.text}}
"#,
    "",
  );
  fs::write(root.path().join(".tasks/child.task"), "options: [{name: item, required: true}]\nsteps: [{file: {operation: write, path: child, content: {parameter: item}}}]\n").unwrap();
  let output = run(root.path(), &["refresh"]);
  assert!(output.status.success(), "{output:?}");
  assert_eq!(fs::read_to_string(root.path().join("child")).unwrap(), "literal names work");
  assert_eq!(fs::read_to_string(root.path().join("outer")).unwrap(), "literal names work");
  assert_eq!(fs::read_to_string(root.path().join("selected")).unwrap(), "selected");
  assert!(fs::read_to_string(root.path().join("page.md"))
    .unwrap()
    .contains("result: \"literal\""));
}

#[test]
fn text_download_seeds_empty_body_and_exposes_body_state() {
  let root = tempfile::tempdir().unwrap();
  let readme = "# Title\n\nAll text, including Caf\u{e9}.\n\n## Details\n\n```rust\n// keep this comment\n```\n";
  let (url, worker) = fixture(vec![(200, String::new(), readme.into())]);
  setup(
    root.path(),
    r#"steps:
  - id: remote
    http: {method: GET, url: {setting: url}, response-format: text}
  - id: document
    file:
      operation: frontmatter
      path: page.md
      create-if-missing: true
      initial: {title: Local}
      body-if-empty: {content: {result: remote.data}, strip-first-h1: true}
  - when: {result: document.body-empty}
    file: {operation: write, path: wrong, content: wrong}
"#,
    &url,
  );
  let output = run(root.path(), &["refresh"]);
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  assert!(fs::read_to_string(root.path().join("page.md"))
    .unwrap()
    .ends_with(&readme.replacen("# Title\n", "", 1)));
  assert!(!root.path().join("wrong").exists());
  assert_eq!(worker.join().unwrap().len(), 1);
}

#[test]
fn legacy_text_requires_explicit_fallback_and_valid_utf8_is_never_reinterpreted() {
  for (bytes, fallback, expected) in [
    (b"column \xa6 next".to_vec(), false, None),
    (b"column \xa6 next".to_vec(), true, Some("column \u{a6} next")),
    ("Caf\u{e9}".as_bytes().to_vec(), true, Some("Caf\u{e9}")),
  ] {
    let root = tempfile::tempdir().unwrap();
    let (url, worker) = fixture_bytes(vec![(200, String::new(), bytes)]);
    let option = if fallback {
      ", fallback-encoding: windows-1252"
    } else {
      ""
    };
    setup(root.path(), &format!("steps:\n  - id: remote\n    http: {{method: GET, url: {{setting: url}}, response-format: text{option}}}\n  - file: {{operation: write, path: downloaded, content: {{result: remote.data}}}}\n"), &url);
    let output = run(root.path(), &["refresh"]);
    assert_eq!(output.status.success(), expected.is_some(), "{output:?}");
    if let Some(expected) = expected {
      assert_eq!(fs::read_to_string(root.path().join("downloaded")).unwrap(), expected);
    } else {
      assert!(!root.path().join("downloaded").exists());
    }
    worker.join().unwrap();
  }
}

#[test]
fn optional_http_only_accepts_missing_resources_and_text_cannot_paginate() {
  for (status, succeeds) in [(404, true), (503, false)] {
    let root = tempfile::tempdir().unwrap();
    let (url, worker) = fixture(vec![(status, String::new(), "not JSON".into())]);
    setup(
      root.path(),
      r#"steps:
  - id: remote
    http: {method: GET, url: {setting: url}, response-format: json, optional: true}
  - when: {result: remote.found}
    file: {operation: write, path: wrong, content: wrong}
"#,
      &url,
    );
    let output = run(root.path(), &["refresh"]);
    assert_eq!(output.status.success(), succeeds, "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!root.path().join("wrong").exists());
    worker.join().unwrap();
  }
  for extra in [
    "response-format: text",
    "response-format: json, optional: true",
  ] {
    let root = tempfile::tempdir().unwrap();
    setup(root.path(), &format!("steps: [{{http: {{method: GET, url: 'http://127.0.0.1:1', {extra}, pagination: {{mode: link-header, relation: next}}}}}}]"), "");
    assert!(!run(root.path(), &["help", "refresh"]).status.success());
  }
}

#[test]
fn body_fill_preserves_encoding_final_newlines_and_concurrent_edit_guards() {
  for encoding in ["utf8", "utf8-bom", "utf16-le", "utf16-be"] {
    let root = tempfile::tempdir().unwrap();
    let original = "---\r\n# retain\r\ntitle: 'My title'\r\n---\r\n\r\n";
    let encode = |text: &str| -> Vec<u8> {
      match encoding {
        "utf8-bom" => [b"\xef\xbb\xbf".as_slice(), text.as_bytes()].concat(),
        "utf16-le" => [
          vec![0xff, 0xfe],
          text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat(),
        "utf16-be" => [
          vec![0xfe, 0xff],
          text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        ]
        .concat(),
        _ => text.as_bytes().to_vec(),
      }
    };
    let page = root.path().join("page.md.disabled");
    fs::write(&page, encode(original)).unwrap();
    setup(
      root.path(),
      r#"steps:
  - file:
      operation: frontmatter
      path: page.md.disabled
      body-if-empty: {content: "Heading\n=======\n\nText.\n", strip-first-h1: true}
"#,
      "",
    );
    let output = run(root.path(), &["refresh"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read(&page).unwrap(), encode(&format!("{original}\r\nText.\r\n")));
    let modified = fs::metadata(&page).unwrap().modified().unwrap();
    assert!(run(root.path(), &["refresh"]).status.success());
    assert_eq!(fs::metadata(&page).unwrap().modified().unwrap(), modified);
    let saved = fs::read(&page).unwrap();
    setup(root.path(), "steps: [{file: {operation: frontmatter, path: page.md.disabled, expected-sha256: stale, body-if-empty: {content: wrong}}}]", "");
    assert!(!run(root.path(), &["refresh"]).status.success());
    assert_eq!(fs::read(&page).unwrap(), saved);
  }
}
