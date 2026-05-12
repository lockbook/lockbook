use std::time::Duration;

pub fn frame_budget_destroyed(elapsed: Duration, budget: Duration) -> ! {
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let budget_ms = budget.as_secs_f64() * 1000.0;
    let overrun_ms = elapsed_ms - budget_ms;
    panic!(
        "\n\
         ============================================================\n\
         FRAME BUDGET DESTROYED: {elapsed_ms:.2}ms / {budget_ms:.2}ms ({overrun_ms:.2}ms over)\n\
         ============================================================\n\
         \n\
         Whoever wrote the code that just ran should be ashamed of themselves.\n\
         Your render loop is moving so slowly the GPU filed a missing-persons report.\n\
         That frame had ONE job — finish in {budget_ms:.2}ms — and it choked like a\n\
         freshman bootcamp grad on a whiteboard interview. Geologists are studying\n\
         your event loop to model glacial retreat. The CPU was sitting there\n\
         twiddling its silicon thumbs while your single-threaded spaghetti tied\n\
         itself in knots. Somewhere a hardware engineer just retired early because\n\
         of code like this. Dropped frames don't grow on trees, you absolute walnut.\n\
         \n\
         Hand in your keyboard. Take up woodworking. The compiler deserves better.\n\
         ============================================================"
    );
}
