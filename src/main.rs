use rocket::{
    get, post,
    serde::{json::Json, Deserialize, Serialize},
};

use rocket_okapi::{
    gen::OpenApiGenerator,
    openapi, openapi_get_routes,
    request::{OpenApiFromRequest, RequestHeaderInput},
    swagger_ui::*,
    JsonSchema,
};

use rocket_sync_db_pools::{database, rusqlite};

#[derive(Deserialize, Serialize, JsonSchema)]
struct Todo {
    id: i64,
    task: String,
    done: bool,
}

impl<'a> TryFrom<&'a rusqlite::Row<'a>> for Todo {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> rusqlite::Result<Todo> {
        Ok(Todo {
            id: row.get(0)?,
            task: row.get(1)?,
            done: row.get(2)?,
        })
    }
}

#[database("sqlite_todos")]
struct TodoDbConn(rusqlite::Connection);

impl TodoDbConn {
    async fn load_todo(&self, id: i64) -> rusqlite::Result<Todo> {
        self.run(move |c| {
            let mut stmt = c.prepare("SELECT id, task, done FROM todos WHERE id = ?1")?;
            stmt.query_row([id], |row| row.try_into())
        })
        .await
    }

    async fn load_todos(&self) -> rusqlite::Result<Vec<Todo>> {
        self.run(|c| {
            let mut stmt = c.prepare("SELECT id, task, done FROM todos")?;
            let todo_iter = stmt.query_map([], |row| row.try_into())?;
            todo_iter.collect()
        })
        .await
    }

    async fn save_todo(&self, task: String) -> rusqlite::Result<Todo> {
        self.run(|c| {
            c.execute("INSERT INTO todos (task, done) VALUES (?1, false)", [&task])?;
            Ok(Todo {
                id: c.last_insert_rowid(),
                task,
                done: false,
            })
        })
        .await
    }
}

impl<'r> OpenApiFromRequest<'r> for TodoDbConn {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}

/// Gets all todo items
#[openapi]
#[get("/todo")]
async fn get_todos(conn: TodoDbConn) -> Option<Json<Vec<Todo>>> {
    conn.load_todos().await.ok().map(Json)
}

/// Gets the todo item with the specified ID
#[openapi]
#[get("/todo/<id>")]
async fn get_todo(conn: TodoDbConn, id: i64) -> Option<Json<Todo>> {
    conn.load_todo(id).await.ok().map(Json)
}

/// Creates a new todo item with the given description
#[openapi]
#[post("/todo", data = "<task>")]
async fn new_todo(conn: TodoDbConn, task: String) -> Option<Json<Todo>> {
    conn.save_todo(task).await.ok().map(Json)
}

#[rocket::launch]
fn rocket() -> _ {
    rocket::build()
        .attach(TodoDbConn::fairing())
        .mount("/api", openapi_get_routes![get_todo, get_todos, new_todo])
        .mount(
            "/swagger-ui/",
            make_swagger_ui(&SwaggerUIConfig {
                url: "../api/openapi.json".into(),
                ..Default::default()
            }),
        )
}
