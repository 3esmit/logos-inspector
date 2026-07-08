{ lib, root }:

let
  workspaceRoot = toString root;

  relativeToRoot = path:
    let
      pathString = toString path;
      prefix = workspaceRoot + "/";
    in
    if pathString == workspaceRoot then "" else lib.removePrefix prefix pathString;

  sourceFilter = path: type:
    let
      name = builtins.baseNameOf path;
      ignoredDirectories = [
        ".direnv"
        "coverage"
        "dist"
        "node_modules"
        "target"
        "tmp"
      ];
      isIgnoredDirectory = type == "directory" && builtins.elem name ignoredDirectories;
      isHiddenDirectory = type == "directory" && lib.hasPrefix "." name;
      isResultLink = name == "result" || lib.hasPrefix "result-" name;
      isLogFile = lib.hasSuffix ".log" name;
    in
    lib.cleanSourceFilter path type
    && !isIgnoredDirectory
    && !isHiddenDirectory
    && !isResultLink
    && !isLogFile;

  isRuntimeAsset = rel:
    rel == "qml" || lib.hasPrefix "qml/" rel
    || rel == "icons" || lib.hasPrefix "icons/" rel;

  isRuntimeAssetSourcePath = rel:
    rel == "" || isRuntimeAsset rel;
in
{
  inherit relativeToRoot sourceFilter;

  workspace = lib.cleanSourceWith {
    src = root;
    filter = sourceFilter;
  };

  standaloneRust = lib.cleanSourceWith {
    src = root;
    filter = path: type:
      sourceFilter path type && !isRuntimeAsset (relativeToRoot path);
  };

  standaloneAssets = lib.cleanSourceWith {
    src = root;
    filter = path: type:
      sourceFilter path type && isRuntimeAssetSourcePath (relativeToRoot path);
  };

  coreModule = lib.cleanSourceWith {
    src = root + "/core";
    filter = sourceFilter;
  };
}
