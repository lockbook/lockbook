import 'package:client/editor.dart';
import 'package:client/either.dart';

import 'errors.dart';

class FileDescription {
  final String id;
  final String name;
  final String path;
  final int version;

  FileDescription(this.id, this.name, this.path, this.version);

  bool _isValid() {
    return id != null && name != null && path != null && version != null;
  }

  static Either<UIError, FileDescription> fromMap(Map<String, dynamic> input) {
    final user = FileDescription(
      input['id'] as String,
      input['name'] as String,
      input['path'] as String,
      input['version'] as int,
    );

    EditorPage(null);

    if (user._isValid()) {
      return Success(user);
    } else {
      return Fail(
          UIError('Could not deserialize user', 'Could not serialize user'));
    }
  }
}
