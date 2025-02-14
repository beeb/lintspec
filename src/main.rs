fn main() {
    dotenvy::dotenv().ok(); // load .env file if present

    println!("Hello, world!");
}
