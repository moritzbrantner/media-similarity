import { Camera, Cloud, Film, FolderPlus, HardDrive } from "lucide-react";

export function sourceKindIcon(kind: string) {
  switch (kind) {
    case "camera":
      return Camera;
    case "minio":
    case "s3":
      return Cloud;
    case "video":
      return Film;
    case "local":
      return HardDrive;
    default:
      return FolderPlus;
  }
}
