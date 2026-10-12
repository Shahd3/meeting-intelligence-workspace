use super::*;

fn workspace() -> Workspace {
    Workspace {
        id: 0,
        name: "Test workspace".into(),
        description: "".into(),
        color: "sage".into(),
        symbol: "✳".into(),
        overview: "".into(),
        decision: None,
        task: "Review".into(),
        owner: "".into(),
        due: "".into(),
    }
}
fn meeting(project: i64) -> Meeting {
    Meeting {
        id: 0,
        title: "Test meeting".into(),
        project,
        date: "Oct 11, 2026".into(),
        duration: "00:01".into(),
        summary: "Synthetic notes".into(),
        highlights: vec!["Highlight".into()],
        decision: "Decision".into(),
        actions: vec!["Review".into()],
        speakers: vec!["Test".into()],
        recorded: false,
    }
}
fn memory() -> Connection {
    open(Path::new(":memory:")).unwrap()
}

#[test]
fn workspace_crud_and_parameterized_text() {
    let mut db = memory();
    let mut w = workspace();
    w.name = "O'Brien'); DROP TABLE meetings; --".into();
    let data = apply(&mut db, Request::CreateWorkspace { workspace: w }).unwrap();
    let mut w = data.workspaces[0].clone();
    assert!(w.id > 0);
    w.description = "Edited".into();
    let data = apply(
        &mut db,
        Request::UpdateWorkspace {
            workspace: w.clone(),
        },
    )
    .unwrap();
    assert_eq!(data.workspaces[0].description, "Edited");
    let data = apply(&mut db, Request::DeleteWorkspace { id: w.id }).unwrap();
    assert!(data.workspaces.is_empty());
    assert!(data.meetings.is_empty());
    assert!(apply(&mut db, Request::DeleteWorkspace { id: w.id }).is_err());
}

#[test]
fn meeting_crud_assignment_and_workspace_deletion() {
    let mut db = memory();
    let first = apply(
        &mut db,
        Request::CreateWorkspace {
            workspace: workspace(),
        },
    )
    .unwrap()
    .workspaces[0]
        .id;
    let second = apply(
        &mut db,
        Request::CreateWorkspace {
            workspace: workspace(),
        },
    )
    .unwrap()
    .workspaces[1]
        .id;
    let mut m = apply(
        &mut db,
        Request::CreateMeeting {
            meeting: meeting(first),
        },
    )
    .unwrap()
    .meetings[0]
        .clone();
    m.project = second;
    m.title = "Moved".into();
    assert_eq!(
        apply(
            &mut db,
            Request::UpdateMeeting {
                meeting: m.clone(),
                title_edited: None
            }
        )
        .unwrap()
        .meetings[0]
            .project,
        second
    );
    apply(
        &mut db,
        Request::Complete {
            key: format!("{}-0", m.id),
            value: true,
        },
    )
    .unwrap();
    let data = apply(&mut db, Request::DeleteWorkspace { id: second }).unwrap();
    assert_eq!(data.meetings.len(), 1);
    assert_eq!(data.meetings[0].project, 0);
    assert_eq!(data.meetings[0].summary, "Synthetic notes");
    assert!(data.completed[&format!("{}-0", m.id)]);
    let data = apply(&mut db, Request::DeleteMeeting { id: m.id }).unwrap();
    assert!(data.meetings.is_empty());
    assert!(data.completed.is_empty());
}

#[test]
fn invalid_writes_do_not_change_existing_data() {
    let mut db = memory();
    assert!(apply(
        &mut db,
        Request::CreateMeeting {
            meeting: meeting(999)
        }
    )
    .is_err());
    let mut w = workspace();
    w.name = " ".into();
    assert!(apply(&mut db, Request::CreateWorkspace { workspace: w }).is_err());
    let mut m = apply(
        &mut db,
        Request::CreateMeeting {
            meeting: meeting(0),
        },
    )
    .unwrap()
    .meetings[0]
        .clone();
    apply(
        &mut db,
        Request::Complete {
            key: format!("{}-0", m.id),
            value: true,
        },
    )
    .unwrap();
    m.project = 999;
    assert!(apply(
        &mut db,
        Request::UpdateMeeting {
            meeting: m.clone(),
            title_edited: None
        }
    )
    .is_err());
    let data = apply(&mut db, Request::Load).unwrap();
    assert_eq!(data.meetings[0].project, 0);
    assert!(data.completed[&format!("{}-0", m.id)]);
    m.project = 0;
    m.actions = vec!["Different action".into()];
    assert!(apply(
        &mut db,
        Request::UpdateMeeting {
            meeting: m,
            title_edited: None
        }
    )
    .unwrap()
    .completed
    .is_empty());
    assert!(apply(
        &mut db,
        Request::Complete {
            key: "1-99".into(),
            value: true
        }
    )
    .is_err());
}

#[test]
fn reopen_preserves_data_and_migration_is_repeatable() {
    let directory = std::env::temp_dir().join(format!(
        "miw-db-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("test.sqlite3");
    let recording = directory.join("preserved-audio.wav");
    std::fs::write(&recording, b"synthetic recording marker").unwrap();
    let id;
    {
        let mut db = open(&path).unwrap();
        id = apply(
            &mut db,
            Request::CreateWorkspace {
                workspace: workspace(),
            },
        )
        .unwrap()
        .workspaces[0]
            .id;
        apply(
            &mut db,
            Request::CreateMeeting {
                meeting: meeting(id),
            },
        )
        .unwrap();
        apply(
            &mut db,
            Request::Complete {
                key: format!("workspace-{id}"),
                value: true,
            },
        )
        .unwrap();
    }
    {
        let mut db = open(&path).unwrap();
        let data = apply(&mut db, Request::Load).unwrap();
        assert_eq!(data.meetings[0].project, id);
        assert!(data.completed[&format!("workspace-{id}")]);
        let data = apply(&mut db, Request::DeleteWorkspace { id }).unwrap();
        assert_eq!(data.meetings[0].project, 0);
        assert!(data.completed.is_empty());
        assert!(recording.exists());
    }
    std::fs::remove_file(recording).unwrap();
    std::fs::remove_file(path).unwrap();
    // SQLite closes/checkpoints the WAL when the final connection is dropped.
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn future_schema_is_rejected_without_resetting_data() {
    let path = std::env::temp_dir().join(format!(
        "miw-future-{}-{}.sqlite3",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let mut db = open(&path).unwrap();
        apply(
            &mut db,
            Request::CreateMeeting {
                meeting: meeting(0),
            },
        )
        .unwrap();
        db.execute_batch("PRAGMA user_version=10").unwrap();
    }
    assert!(open(&path).is_err());
    {
        let db = Connection::open(&path).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM meetings", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            10
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn desktop_preferences_migrate_without_changing_existing_meetings() {
    let directory =
        std::env::temp_dir().join(format!("miw-preference-migrate-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("test.sqlite3");
    {
        let db = Connection::open(&path).unwrap();
        db.execute_batch(include_str!("../../migrations/0001.sql"))
            .unwrap();
        db.execute_batch(include_str!("../../migrations/0002.sql"))
            .unwrap();
        // Seed the historical schema without the current read model, then migrate.
        db.execute("INSERT INTO meetings(title,date,duration,summary,highlights,decision,actions,speakers) VALUES ('Untitled Meeting','','','','[]','','[]','[]')",[]).unwrap();
    }
    {
        let db = open(&path).unwrap();
        assert!(!super::detection_enabled(&db).unwrap());
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM meetings", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        super::set_detection_enabled(&db, true).unwrap();
    }
    {
        let db = open(&path).unwrap();
        assert!(super::detection_enabled(&db).unwrap());
    }
    std::fs::remove_dir_all(directory).unwrap();
}
