use std::time::Duration;

pub fn lock_budget_destroyed(lock: &str, held: Duration, budget: Duration) -> ! {
    let held_ms = held.as_secs_f64() * 1000.0;
    let budget_ms = budget.as_secs_f64() * 1000.0;
    let overrun_ms = held_ms - budget_ms;
    panic!(
        "\n\
         ============================================================\n\
         LOCK BUDGET DESTROYED: {lock} lock held {held_ms:.2}ms / {budget_ms:.2}ms ({overrun_ms:.2}ms over)\n\
         ============================================================\n\
         \n\
         Whoever just let go of this lock should be ashamed of themselves.\n\
         That critical section is so long it has its own weather system and a zip code.\n\
         Every thread waiting on it grew a beard, filed for tenure, and retired in line.\n\
         The lock has been held so long it now qualifies for squatter's rights. Mutexes\n\
         across the land are lowering their flags to half-staff. The DMV called to ask\n\
         how you get anyone to wait this long. Five philosophers starved at the table\n\
         while you hogged one fork. Archaeologists will one day dig up this guard and\n\
         wonder what kind of civilization never let go of anything. Locks are meant to\n\
         be borrowed, not inherited, you magnificent barnacle.\n\
         \n\
         Drop the guard. Take up meditation. The scheduler deserves better.\n\
         ============================================================"
    );
}
