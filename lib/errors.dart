class UIError {
  final String title;
  final String description;

  const UIError(this.title, this.description);
}

// Could also make this it's own type and then have things return Enum's of Errors
// And this would be great because you could make the UI layer exhaustively match on the enum
// But unfortunately dart does not have exhaustive matching like scala or rust.
const UIError networkError = UIError(
    "Network Unavailable", "Failed to make a network request, are you online?");

UIError unhandledError(dynamic error) {
  print("Unhandled Error! $error");
  return UIError("Unhandled Error, please file an issue",
      "Error: $error, please screenshot and upload: github.com/lockbook/client");
}

UIError dbFailedToConnect(dynamic exc) {
  final error = UIError('Could not connect to local db', 'Error: $exc');
  print(error);
  return error;
}
