import 'main.dart';

class UIError {
  final String title;
  final String description;

  const UIError(this.title, this.description);
}

UIError networkError() => logPassThrough(
      UIError("Network Unavailable",
          "Failed to make a network request, are you online?"),
    );

UIError pathProviderError() => logPassThrough(
      UIError(
          "Unable to access file system",
          "It seems path_provider is not supported on this platform, "
              "please tell us what platform you're using, and we'll investigate: "
              "github.com/lockbook -> issues"),
    );

UIError couldNotCreateFileFolder(dynamic error) => logPassThrough(
      UIError('Could not create file Folder!',
          'File folder does not exist, and upon attempting to create it we got tihs message: $error'),
    );

UIError unhandledError(dynamic error, dynamic initiaStackTrace) {
  logger.e('Uncaught error!', initiaStackTrace);
  return UIError("Unhandled Error, please file an issue",
      "Error: $error, please screenshot and upload: github.com/lockbook/client");
}

UIError dbFailedToConnect(dynamic exc) => logPassThrough(
      UIError('Could not connect to local db', 'Error: $exc'),
    );

UIError fileWriteError(String location, dynamic exc) => logPassThrough(
      UIError(
          "Could not write to file", "Error: $exc while writing to $location"),
    );

UIError fileReadError(String location, dynamic exc) => logPassThrough(
      UIError("Could not read file", "Error: $exc while writing to $location"),
    );

UIError versionParseError(String version, dynamic exc) => logPassThrough(
      UIError('Error Parsing File Version',
          '$version could not be parsed as a BigInt, error: $exc'),
    );

UIError fileDescriptionParseError(Map<String, dynamic> input) => logPassThrough(
      UIError('Error parsing file description',
          '$input could not be decoded into a FileDescription'),
    );

UIError userDecodingError(dynamic exc) => logPassThrough(
      UIError('Unable to decode User',
          'Local data seems to be corrupt, error: $exc'),
    );

UIError unknownDbConnectionError() => logPassThrough(
      UIError("Could not connect to your local db", "Unknown error occured"),
    );

UIError usernameUnavailable() => logPassThrough(
      UIError("Username Unavailable", "Please select a different username"),
    );

UIError serverUnavailable() => logPassThrough(
      UIError("Server Unavailable",
          "Please check status.lockbook.app or try again"),
    );

UIError failedToSaveUserInDB() => logPassThrough(
      UIError("Failed to save user info", "Failed to save private key to db!"),
    );

UIError noUserError() => logPassThrough(
      UIError("No User Info saved", "Please create a user"),
    );

UIError failedToInsertFile(String uuid, String name, String path) =>
    logPassThrough(
      UIError('Failed to insert',
          'Failed to insert $uuid, $name, $path, 0 into FileIndex'),
    );

UIError fileNotFound(String path, String name) => logPassThrough(
      UIError("File not found", "No file matches $path, $name"),
    );

UIError logPassThrough(UIError error) {
  logger.e("Error encountered: ${error.title}\t${error.description}", null,
      StackTrace.current);
  return error;
}
