# lbdev
- build workspace
- run lockbook on annoying platforms
- release operations

install: `cargo install --path utils/lbdev`
update: `lbdev update`
completions for fish: `lbdev fish-completions`.

# Release Ops
We release often, every release generally we release everything, and everything has the same version. Our version encodes the date of the batch of changes in `yy.mm.dd` format. We update this often enough so our server can accurately estimate how much usage a particular batch of code is receving. During the development cycle (like on days of release) we may have to increment this more than once in a day, and on these dates we'll just increment the patch field (to effectively the following day's date). Incrementing this often enough also allows us to more clearly distinguish between the code engineers are running on `master` vs. the code that's released to consumers.