import 'package:client/task.dart';

import 'errors.dart';

class FileDescription {
  final String id;
  final String name;
  final String path;
  final int version;

  FileDescription(this.id, this.name, this.path, this.version);

  _isValid() {
    return id != null && name != null && path != null && version != null;
  }

  static Either<UIError, FileDescription> fromMap(Map input) {
    final user = FileDescription(
        input['id'], input['name'], input['path'], input['version']);

    if (user._isValid()) {
      return Success(user);
    } else {
      return Fail(
          UIError('Could not deserialize user', 'Could not serialize user'));
    }
  }
}
