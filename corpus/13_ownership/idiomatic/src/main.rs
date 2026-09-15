//! 13. Ownership transfer: by-value parameters, consuming receivers, a builder, explicit clones.

#[derive(Debug, Clone)]
struct Job {
    id: u32,
    payload: String,
}

#[derive(Debug, Default)]
struct Queue {
    jobs: Vec<Job>,
}

impl Queue {
    fn enqueue(&mut self, job: Job) {
        self.jobs.push(job);
    }

    fn drain(self) -> Vec<Job> {
        self.jobs
    }
}

struct RequestBuilder {
    url: String,
    headers: Vec<(String, String)>,
}

impl RequestBuilder {
    fn new(url: &str) -> Self {
        RequestBuilder {
            url: url.to_string(),
            headers: Vec::new(),
        }
    }

    fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    fn build(self) -> String {
        let mut out = format!("GET {}", self.url);
        for (k, v) in self.headers {
            out.push_str(&format!("\n{k}: {v}"));
        }
        out
    }
}

fn consume(text: String) -> usize {
    text.len()
}

fn main() {
    let mut queue = Queue::default();
    let job = Job {
        id: 1,
        payload: String::from("data"),
    };
    queue.enqueue(job.clone());
    queue.enqueue(job);
    let jobs = queue.drain();
    println!(
        "{} jobs, first #{} {:?}",
        jobs.len(),
        jobs[0].id,
        jobs[0].payload
    );
    let request = RequestBuilder::new("/index")
        .header("Host", "example.org")
        .header("Accept", "*/*")
        .build();
    println!("{request}");
    let text = String::from("moved into consume");
    println!("{}", consume(text));
}
