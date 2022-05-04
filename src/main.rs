use web_crawler::box_crawl;

fn main() {
    async_std::task::block_on(async {
        let args: Vec<String> = std::env::args().collect::<Vec<String>>();
        let website_name = &args[1];
        let depth_max: u8 = args[2].parse::<u8>().unwrap();

        let _ = box_crawl(vec![url::Url::parse(website_name).unwrap()], 1, depth_max).await;
    });
}
