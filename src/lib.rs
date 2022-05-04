use std::borrow::Borrow;
use html5ever::tokenizer::{
    BufferQueue,
    Tag,
    TagKind,
    TagToken,
    Token,
    TokenSink,
    TokenSinkResult,
    Tokenizer,
    TokenizerOpts,
};
use async_std::task;
use core::panic;
use url::{
    ParseError,
    Url,
};
use surf;

// Send is used so Result can be sent across threads
// Sync is used so Result can shared references across threads
// Static lifetime is necessary to be able to be return by an async function
type CrawlResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;
type BoxFuture = std::pin::Pin<Box<dyn std::future::Future<Output = CrawlResult> + Send>>;

// Vecotr will hold a list of all the website links we will crawl through
#[derive(Default, Debug)]
struct LinkQueue {
    links: Vec<String>,
}

// Used to get the correct url string HTML tag
impl TokenSink for &mut LinkQueue{
    type Handle = ();
    // HTML tags looks like this: <a href="link">some text</a>
    // we are looking into the start tag <a {....} >
    fn process_token(
        &mut self, 
        token: Token, 
        _line_number: u64
    ) -> TokenSinkResult<Self::Handle> {
        match token {
            TagToken(
                ref tag @ Tag {
                    kind: TagKind::StartTag,
                    ..
                },
            ) => {
                // See if tag name is "a (opening tag) and iterate
                // through attributes to find url string
                if tag.name.as_ref() == "a" {
                    for attribute in tag.attrs.iter() {
                        if attribute.name.local.as_ref() == "href" {
                            let url_str: &[u8] = attribute.value.borrow();
                            self.links.push(
                                String::from_utf8_lossy(url_str).into_owned());
                        }
                    }
                }
            },
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

// Putting Crawl() into a Box pointer to be more effecient
pub fn box_crawl(pages: Vec<Url>, current: u8, max: u8) -> BoxFuture {
    Box::pin(crawl(pages, current, max))
}

// Takes in a url and a page
// Returns a vector of urls
fn get_links(url: &Url, page: String) -> Vec<Url> {
    let mut domain_url = url.clone();
    // Removing path and query from url
    // e.g "https://www.example.com/api/example?query=10"
    // -> "https://www.example.com"
    domain_url.set_path("");
    domain_url.set_query(None);

    let mut queue = LinkQueue::default();
    // Identifies the tokens in the HTML5 document
    let mut tokenizer = Tokenizer::new(&mut queue, TokenizerOpts::default());
    // Allows to go through the page string one at a time
    let mut buffer = BufferQueue::new();
    buffer.push_back(page.into());
    let _ = tokenizer.feed(&mut buffer);

    // Iterate through all the links to check if they contain a valid url.
    return queue.links.iter()
        .map(|link| match Url::parse(link){
            Ok(url) => url,
            Err(ParseError::RelativeUrlWithoutBase) => domain_url.join(link).unwrap(),
            Err(_) => panic!("Malformed link found: {}", link)})
        .collect::<Vec<Url>>();
}

// Takes in a vector of urls (pages), current depth, max depth.
// Returns a future type that's wrapped around a CrawlResult.
// Depths determine how many pages from the original page that was called,
// do we want to continue to crawl.
// Going through each of the urls, getting them from the pages, and spawn
// tasks for each url, so we can continue to follow them
async fn crawl(pages: Vec<Url>, current: u8, max:u8) -> CrawlResult {
    println!("Current Depth: {current}, Max Depth: {max}");

    // If max depth is reached return with Ok
    if current > max {
        println!("Reached Max Depth!!");
        return Ok(());
    }

    // Vector that holds the different tasks for each url
    let mut tasks = vec![];
    println!("crawling: {:?}", pages);

    // Spawn the different tasks and push them into the tasks vector
    for url in pages {
        // move keyword will move all of the outside variables into this context
        let task = task::spawn(async move {
            println!("getting: {url}");
        
            let mut res = surf::get(&url).await?;
            let body = res.body_string().await?;

            let links = get_links(&url, body);

            println!("Following: {:?}", links);
            // To continue to follow the the urls, we need to recursively call
            // Crawl(), after we get all of the links, on each of the links
            box_crawl(links, current + 1, max).await
        });
        tasks.push(task);
    }

    // Have to call await to each task to execute them
    for task in tasks.into_iter() {
        task.await?;
    }
    return Ok(())
}