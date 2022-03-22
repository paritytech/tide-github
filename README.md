# tide-github

Process Github webhooks in [tide](https://github.com/http-rs/tide).

[API docs](https://docs.rs/tide-github/0.1.0/tide_github/)

```Rust
use tide_github::Event;

#[async_std::main]
async fn main() -> tide::Result<()> {
    let mut app = tide::new();
    let github = tide_github::new(b"My Github webhook s3cr#t")
        .on(Event::IssueComment, |payload| {
            println!("Got payload for repository {}", payload.repository.name);
        })
        .build();
    app.at("/gh_webhooks").nest(github);
    app.listen("127.0.0.1:3000").await?;
    Ok(())
}
```
