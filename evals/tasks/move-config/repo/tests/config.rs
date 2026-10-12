use tinyapp::config::Config;
use tinyapp::describe;

#[test]
fn new_starts_with_one_worker() {
    assert_eq!(Config::new("api").workers, 1);
}

#[test]
fn describe_mentions_the_workers() {
    assert_eq!(describe(&Config::new("api")), "api with 1 worker(s)");
}
