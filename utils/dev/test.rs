ch1 := new_channel();
ch2 := new_channel();

let handle = thread::spawn(search(ch1.rx, ch2.rx));

// TX read all the docs, create a struct move the channels and content into struct close the tx. 
// this is different than every function in core
//
// Debouncing for every command you hold some sort of timestamp and then when a certain amount of
// time ellapses you actually execute that command
// Killing <----
fn search(tx, rx) -> Result<(), Error> {
    thread::join(thread::spawn(do_search)) // move channels into search
}

fn do_search(tx, rx) { 
    let a = rx.recv();

    let a_search = thread::spawn(do the search here with the info you have)

    let mut last_recvd = a;

    // conceptual debounce
    loop {
        let known_recvd = last_recvd.clone();
        sleep(debounce_duration)

        if last_recvd == known_recvd {
            // safe to do search, debounce done
        }
    }

    loop {
        last_recvd = rx.recv();
    }
    kill(a_search)
    start(b_search)
}
