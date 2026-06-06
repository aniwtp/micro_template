use ntex::web;

use crate::bd::DBWrapper;

pub mod bd;
pub mod generated;
pub mod logic;
pub mod routes;

#[ntex::main]
async fn main() -> Result<(), std::io::Error> {
    let bd = DBWrapper::new("test.redb").unwrap();
    let app = async move || web::App::new().state(bd.clone()).configure(routes::routes);

    web::server(app).bind("localhost:8080")?.run().await
}
