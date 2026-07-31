pub(crate) struct Board {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) threads: Vec<Thread>,
}

pub(crate) struct Thread {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) replies: Vec<Reply>,
}

pub(crate) struct Reply {
    pub(crate) body: String,
}

#[derive(sqlx::FromRow)]
struct BoardRow {
    slug: String,
    name: String,
    description: String,
}

pub(crate) async fn load_approved_boards(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<Board>, sqlx::Error> {
    let rows = sqlx::query_as::<_, BoardRow>(
        r#"
        SELECT slug, name, description
        FROM boards 
        WHERE status = 'approved' 
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Board {
            slug: row.slug,
            name: row.name,
            description: row.description,
            threads: Vec::new(),
        })
        .collect())
}

pub(crate) fn seed_boards() -> Vec<Board> {
    vec![Board {
        slug: String::from("engineering"),
        name: String::from("Engineering"),
        description: String::from(
            "Discussion for the Engineering
 Faculty.",
        ),
        threads: vec![
            Thread {
                id: 1,
                title: String::from("Welcome to Engineering"),
                body: String::from("Introduce yourself and share useful resources."),
                replies: vec![
                    Reply {
                        body: String::from("Glad to be here."),
                    },
                    Reply {
                        body: String::from("Looking forward to meeting everyone."),
                    },
                ],
            },
            Thread {
                id: 2,
                title: String::from("Study group ideas"),
                body: String::from("What subjects should we organize study groups for?"),
                replies: vec![],
            },
        ],
    }]
}
