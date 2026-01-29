+++
title = "The story of how Lockbook created its own database for speed and productivity"
date = 2023-04-19
[extra]
author= "parth"
author_link = "https://github.com/Parth"
+++


[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F5fa91464-5596-4743-a17d-a324b4f43e8c_1018x994.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F5fa91464-5596-4743-a17d-a324b4f43e8c_1018x994.png)

As a backend engineer, the architecture I see used most commonly is a loadbalancer distributing requests to several horizontally scaled API servers. Those API servers are generally talking to one or more stores of state. [Lockbook](https://parth.cafe/p/introducing-lockbook) also started this way, we load balanced requests using HAProxy, had a handful of [Rust API nodes](https://parth.cafe/p/why-lockbook-chose-rust), and stored our data in Postgres and S3.

A year into the project, we had developed enough of the product that we understood our needs more clearly, but we were still early enough into our journey where we could make breaking changes and run experiments. I had some reservations about this _default_ architecture, and before the team stabilized our API, I wanted to see if we could do better.

My first complaint was about our interactions with SQL. It was annoying to shuffle data back and forth from the fields of our structs into columns of our tables. Over time our SQL queries grew more complicated, and it was hard to express and maintain ideas like _a user's file tree cannot have cycles_ or _a file cannot have the same name as a non-deleted sibling_. We were constantly trying to determine whether we should express something in SQL, or read a user's data into our API server, perform and validate the operation in Rust, and then save the new state of their file tree. Concerns around transaction isolation, consistency, and performance were always hard to reason about. We were growing frustrated because we knew how we want this data to be stored and processed and were burning cycles fighting our declarative environment.

My second complaint was about how much infrastructure we had to manage. While on the topic of Postgres itself, running Postgres at a production scale is not trivial. There's a great deal of trivia you have to understand to make Postgres work properly with your API servers and your hardware. First we had to understand what features of Postgres our database libraries supported. In our case, that meant evaluating whether we needed to additionally run PGBouncer, Postgres' connection pooling server, and potentially another piece of infrastructure to manage. Regardless of PGBouncer, configuring Postgres itself requires an understanding of how Postgres interacts with your hardware. From Postgres' [configuration guide](https://wiki.postgresql.org/wiki/Tuning_Your_PostgreSQL_Server):

> PostgreSQL ships with a basic configuration tuned for wide compatibility rather than performance. Odds are good the default parameters are very undersized for your system....

That's just Postgres. Similar complexities existed for S3, HAProxy, and the networking and monitoring considerations of all the nodes mentioned thus far. This was quickly becoming overwhelming, and we hadn't broken ground on _user collaboration_ , one of our most ambitious features. For a team sufficiently large this may be no big deal. Just hire some ops people to stand up the servers so the software engineers can engineer the software. For our resource-constrained team of 5, this wasn't going to work. Additionally, when we surveyed the useful work our servers were performing, we knew this level of complexity was unnecessary.

For example, when a user signs up for Lockbook or makes an edit to a file, the actual useful work that our server did to record that information should have taken no more than 2ms. But from our load balancer's reporting, those requests were taking 50-200 ms. We were using all these heavy-weight tools to be able to field lots of concurrent requests without paying any attention to how long those requests were taking. Would we need all this if the requests were fast?

We ran some experiments with Redis and stored files in EBS instead of S3, and the initial results were promising. We expressed all our logic in Rust and vastly increased the amount of code we were able to share with our clients (core). We dramatically reduced our latency, and our app felt noticeably faster. However, most of that request time was spent waiting for Redis to respond over the network (even if we hosted our application and database on the same server). And we were still spending time ferrying information in and out of Redis. I knew something was interesting to explore here.

So after a week of prototyping, I created [db-rs](https://github.com/parth/db-rs). The idea was to make a stupid-simple database that could be embedded as a Rust library directly into our application. No network hops, no context switches, and huge performance gains. Have it be easy for someone to specify a schema in Rust, and allow them to pick what the performance characteristics of these simple key-value style tables would be. This is Core's schema, for instance:
    
    
    #[derive(Schema, Debug)]
    pub struct CoreV3 {
        pub account: Single<Account>,
        pub last_synced: Single<i64>,
        pub root: Single<Uuid>,
        pub local_metadata: LookupTable<Uuid, SignedFile>,
        pub base_metadata: LookupTable<Uuid, SignedFile>,
        pub pub_key_lookup: LookupTable<Owner, String>,
        pub doc_events: List<DocEvent>,
    }
    

The types `Single`, `LookupTable`, and `List` are db-rs table types. They are backed by Rust `Option`, `HashMap`, or `Vec` respectively. They capture changes to their data structures, `Serialize` those changes and append them to the end of a log -- one of the fastest ways to persist an event.

The types `Account`, `SignedFile`, `Uuid`, etc are types Lookbook is using. They all implement the ubiquitous `Serialize` `Deserialize` traits, so we never again need to think about converting between our types and their on-disk format. Internally db-rs uses `bincode` format, an incredibly [performant](https://github.com/djkoloski/rust_serialization_benchmark) and compact representation of your data.

What's cool here is that when you query out of a table, you're handed pointers to _your data_. The database isn't fetching bytes, serializing them, or sending them over the wire for your program to then shuffle into its fields. A read from one of these tables is a direct memory access, and because of Rust's memory guarantees, you can be sure that reference will be valid for the duration of your access to it.

What's exciting from an ergonomics standpoint is that your schema is statically known by your editor. It's not defined and running on a server somewhere else. So if you type `db.` you get a list of your tables. If you select one, then that table-type's contract is shown to you, with _your_ keys and values. Additionally for us, now our backend stack doesn't require any container orchestration whatsoever: you just need `cargo` to run our server. This has been massive boon for quickly setting up environments whether locally or in production.

The core ideas of the database are less than 800 lines of code and are fairly easy to reason about. This is a database that's working well for us not because of what it does, but because of all the things it _doesn't do_. And what we've gained from db-rs is a tremendous amount of performance and productivity.

Ultimately this is a different way to think about scaling a backend. When you string together 2-4 pieces of infrastructure over the network, you're incurring a big latency cost, and hopefully what you're gaining as a result is availability. But are you? If you're using something like Postgres, you're also in a situation where your database is your single point of failure. You've just surrounded that database with a lot of ceremonies, and I'm skeptical that the ceremony helps Postgres respond to queries faster or that it helps engineers deliver value more quickly.

db-rs has been running in production for half a year at this point. Most requests are replied to in less than 1 ms. We anticipate that on a modest EC2 node, we should be able to scale to hundreds of thousands of users and field hundreds of requests per second. Should we need to, we can scale vertically 1-2 orders of magnitude beyond this point. Ultimately our backend plans to follow a scaling strategy similar to email where users have a home server. And our long-term vision is one of a network of decentralized server operators. But that's a dream that's still quite far away.

As a result, what Lockbook ultimately converged on, is probably my new _default_ approach for building simple backend systems. If this intrigues you, check out the [source code](https://github.com/parth/db-rs) of db-rs or take it for a [spin](https://crates.io/crates/db-rs).

Currently db-rs exactly models the needs of Lockbook. There are key weaknesses around areas of concurrency and offloading seldom accessed data to disk. Whenever Lockbook or one of db-rs' users needs these things, they'll be added. Feel free to open an issue or pull request!
