import { Badge } from "@moritzbrantner/ui";
import { Button } from "@moritzbrantner/ui";
import { Input } from "@moritzbrantner/ui";
import { Label } from "@moritzbrantner/ui";
import { Loader2, Save, X } from "lucide-react";
import { useEffect, useState } from "react";
import type { SearchResult } from "../../types";

export function MediaTagEditor({
  image,
  onUpdateTags,
  saving,
}: {
  image: SearchResult["image"];
  onUpdateTags?: (id: string, tags: string[]) => void;
  saving: boolean;
}) {
  const tags = image.tags ?? [];
  const [draft, setDraft] = useState(tags.join(", "));

  useEffect(() => {
    setDraft(tags.join(", "));
  }, [image.id, tags.join("\u0000")]);

  const draftTags = parseTagDraft(draft);
  const dirty = !sameTags(draftTags, tags);

  function removeTag(tag: string) {
    setDraft(draftTags.filter((item) => item !== tag).join(", "));
  }

  return (
    <form
      className="grid gap-2 rounded-md border border-neutral-200 bg-neutral-50 p-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (onUpdateTags && dirty && !saving) {
          onUpdateTags(image.id, draftTags);
        }
      }}
    >
      <div className="flex items-center justify-between gap-2">
        <Label
          className="text-xs font-semibold uppercase text-neutral-500"
          htmlFor={`tags-${image.id}`}
        >
          Tags
        </Label>
        <Button
          aria-label={`Save tags for ${image.filename}`}
          className="inline-flex h-8 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-2 text-xs font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-100 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={!onUpdateTags || !dirty || saving}
          type="submit"
          variant="outline"
        >
          {saving ? (
            <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <Save className="size-3.5" aria-hidden="true" />
          )}
          <span>Save</span>
        </Button>
      </div>
      <Input
        aria-label={`Tags for ${image.filename}`}
        className="h-9 rounded-md border-neutral-300 bg-white text-sm"
        disabled={!onUpdateTags || saving}
        id={`tags-${image.id}`}
        onChange={(event) => setDraft(event.target.value)}
        placeholder="travel, family"
        value={draft}
      />
      {draftTags.length ? (
        <div className="flex flex-wrap gap-1.5">
          {draftTags.map((tag) => (
            <Badge
              className="inline-flex max-w-full items-center gap-1 rounded-md border border-neutral-300 bg-white px-2 py-1 text-xs font-semibold text-neutral-800"
              key={tag}
              variant="outline"
            >
              <span className="truncate">{tag}</span>
              <button
                aria-label={`Remove tag ${tag}`}
                className="inline-grid size-4 shrink-0 place-items-center rounded text-neutral-500 transition hover:bg-neutral-200 hover:text-neutral-950"
                disabled={!onUpdateTags || saving}
                onClick={() => removeTag(tag)}
                type="button"
              >
                <X className="size-3" aria-hidden="true" />
              </button>
            </Badge>
          ))}
        </div>
      ) : null}
    </form>
  );
}

function parseTagDraft(value: string) {
  const seen = new Set<string>();
  const tags: string[] = [];

  for (const rawTag of value.split(",")) {
    const tag = rawTag.trim();
    const key = tag.toLocaleLowerCase();
    if (!tag || seen.has(key)) {
      continue;
    }
    seen.add(key);
    tags.push(tag);
  }

  return tags;
}

function sameTags(left: string[], right: string[]) {
  return left.length === right.length && left.every((tag, index) => tag === right[index]);
}
