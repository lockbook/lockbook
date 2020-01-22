import 'package:client/either.dart';

import 'errors.dart';

class FileDescription {
  final String id;
  final String name;
  final String path;
  final BigInt version;

  FileDescription(this.id, this.name, this.path, this.version);

  bool _isValid() {
    return id != null && name != null && path != null && version != null;
  }

  static Either<UIError, FileDescription> fromMap(Map<String, dynamic> input) {
    BigInt version;
    try {
      version = BigInt.parse(input['version'] as String);
    } catch (error) {
      return Fail(versionParseError(input['version'] as String, error));
    }

    final user = FileDescription(
      input['id'] as String,
      input['name'] as String,
      input['path'] as String,
      version,
    );

    if (user._isValid()) {
      return Success(user);
    } else {
      return Fail(fileDescriptionParseError(input));
    }
  }
}
