import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";

import { Button } from "../../shared/ui/button";
import { DrawerField } from "../../shared/ui/drawer";
import { Input } from "../../shared/ui/input";
import { MarkdownEditor } from "../shared/ui/MarkdownEditor";
import { ImageField } from "../shared/ui/ImageField";
import {
  jsonOrThrow,
  siteScopedFetch,
  ApiValidationError,
} from "../shared/api";
import { validateEmail, validateDateRange } from "../shared/validation";
import { EditorPreviewDrawer } from "../shared/ui/EditorPreviewDrawer";
import { DraftPreviewPane } from "../shared/ui/DraftPreviewPane";
import {
  ProfileView as ProfileViewCmp,
  type ProfileData,
} from "../../extensions/profile/ProfileView";

interface EducationRow {
  institution: string;
  degree: string;
  field: string;
  start_year: string;
  end_year: string;
}

interface CustomLinkRow {
  label: string;
  url: string;
  icon: string;
}

interface FormState {
  display_name: string;
  tagline_ko: string;
  tagline_en: string;
  avatar_url: string;
  bio_ko: string;
  bio_en: string;
  email: string;
  github_username: string;
  linkedin_url: string;
  education: EducationRow[];
  custom_links: CustomLinkRow[];
}

const EMPTY: FormState = {
  display_name: "",
  tagline_ko: "",
  tagline_en: "",
  avatar_url: "",
  bio_ko: "",
  bio_en: "",
  email: "",
  github_username: "",
  linkedin_url: "",
  education: [],
  custom_links: [],
};

export function ProfileTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [updatedAt, setUpdatedAt] = useState("");
  const [staleNotice, setStaleNotice] = useState<{ local: FormState; remote: ProfileData } | null>(
    null,
  );
  const [dirty, setDirty] = useState(false);

  const load = async () => {
    const res = await siteScopedFetch(slug, "/profile");
    if (res.status === 404) {
      setUpdatedAt("");
      setForm(EMPTY);
      setDirty(false);
      return;
    }
    const body = await jsonOrThrow<{ data: ProfileData }>(res);
    setUpdatedAt(body.data.updated_at);
    setForm({
      display_name: body.data.display_name,
      tagline_ko: body.data.tagline_ko ?? "",
      tagline_en: body.data.tagline_en ?? "",
      avatar_url: body.data.avatar_url ?? "",
      bio_ko: body.data.bio_ko ?? "",
      bio_en: body.data.bio_en ?? "",
      email: body.data.email ?? "",
      github_username: body.data.github_username ?? "",
      linkedin_url: body.data.linkedin_url ?? "",
      education: body.data.education.map((e) => ({
        institution: e.institution ?? "",
        degree: e.degree ?? "",
        field: e.field ?? "",
        start_year: e.start_year != null ? String(e.start_year) : "",
        end_year: e.end_year != null ? String(e.end_year) : "",
      })),
      custom_links: body.data.custom_links.map((l) => ({
        label: l.label,
        url: l.url,
        icon: l.icon ?? "",
      })),
    });
    setDirty(false);
  };

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const validate = (): boolean => {
    const e: Record<string, string> = {};
    if (!form.display_name.trim()) e.display_name = "Display name is required";
    const emailErr = validateEmail(form.email);
    if (emailErr) e.email = emailErr;
    for (let i = 0; i < form.education.length; i++) {
      const r = form.education[i];
      const dr = validateDateRange(r.start_year, r.end_year);
      if (dr) e[`education.${i}`] = dr;
    }
    setErrors(e);
    return Object.keys(e).length === 0;
  };

  const save = useMutation({
    mutationFn: async () => {
      if (!validate()) throw new Error("Please fix the highlighted fields.");
      const res = await siteScopedFetch(slug, "/profile", {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          expected_updated_at: updatedAt,
          display_name: form.display_name.trim(),
          tagline_ko: form.tagline_ko || null,
          tagline_en: form.tagline_en || null,
          avatar_url: form.avatar_url || null,
          bio_ko: form.bio_ko || null,
          bio_en: form.bio_en || null,
          email: form.email || null,
          github_username: form.github_username || null,
          linkedin_url: form.linkedin_url || null,
          education: form.education.map((r) => ({
            institution: r.institution || null,
            degree: r.degree || null,
            field: r.field || null,
            start_year: r.start_year ? Number(r.start_year) : null,
            end_year: r.end_year ? Number(r.end_year) : null,
          })),
          custom_links: form.custom_links.map((l) => ({
            label: l.label,
            url: l.url,
            icon: l.icon || null,
          })),
        }),
      });
      if (res.status === 409) {
        // jsonOrThrow throws on non-ok, so read the 409 body directly —
        // it carries the current remote Profile to offer Reload/Compare.
        const body = (await res.json().catch(() => null)) as
          | { error?: { code?: string; message?: string }; data?: ProfileData }
          | null;
        const e = new Error(
          body?.error?.message ?? "stale_profile",
        ) as Error & { remote?: ProfileData };
        if (body?.data) e.remote = body.data;
        throw e;
      }
      return jsonOrThrow<{ data: ProfileData }>(res);
    },
    onSuccess: ({ data }) => {
      qc.invalidateQueries({ queryKey: ["profile"] });
      setUpdatedAt(data.updated_at);
      setDirty(false);
      setOpen(false);
    },
    onError: (e: unknown) => {
      const err = e as { remote?: ProfileData; message: string };
      if (err.remote) {
        setStaleNotice({ local: form, remote: err.remote });
      } else if (e instanceof ApiValidationError) {
        setErrors({ [e.field]: e.message });
      } else {
        setErrors({ _form: err.message ?? "Save failed" });
      }
    },
  });

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <p className="text-sm text-muted">
          {updatedAt ? `Updated ${updatedAt}` : "Not initialized yet"}
        </p>
        <Button size="sm" disabled={!updatedAt} onClick={() => setOpen(true)}>
          <Plus size={14} className="mr-1" /> Edit Profile
        </Button>
      </div>

      <EditorPreviewDrawer
        open={open}
        onClose={() => setOpen(false)}
        title="Profile"
        description="Singleton editor — full-replace PUT"
        dirty={dirty}
        slug={slug}
        editor={
          <div>
            <DrawerField label="Display name" required error={errors.display_name}>
              <Input
                value={form.display_name}
                onChange={(e) => {
                  setDirty(true);
                  setForm((f) => ({ ...f, display_name: e.target.value }));
                }}
              />
            </DrawerField>
            <div className="grid grid-cols-2 gap-3">
              <DrawerField label="Tagline (Korean)">
                <Input
                  value={form.tagline_ko}
                  onChange={(e) => {
                    setDirty(true);
                    setForm((f) => ({ ...f, tagline_ko: e.target.value }));
                  }}
                />
              </DrawerField>
              <DrawerField label="Tagline (English)">
                <Input
                  value={form.tagline_en}
                  onChange={(e) => {
                    setDirty(true);
                    setForm((f) => ({ ...f, tagline_en: e.target.value }));
                  }}
                />
              </DrawerField>
            </div>
            <DrawerField label="Avatar">
              <ImageField
                slug={slug}
                extension="profile"
                value={form.avatar_url}
                onChange={(v) => {
                  setDirty(true);
                  setForm((f) => ({ ...f, avatar_url: v ?? "" }));
                }}
              />
            </DrawerField>

            <DrawerField label="Email" error={errors.email}>
              <Input
                value={form.email}
                onChange={(e) => {
                  setDirty(true);
                  setForm((f) => ({ ...f, email: e.target.value }));
                }}
                placeholder="hello@example.com"
              />
            </DrawerField>
            <div className="grid grid-cols-2 gap-3">
              <DrawerField label="GitHub username">
                <Input
                  value={form.github_username}
                  onChange={(e) => {
                    setDirty(true);
                    setForm((f) => ({ ...f, github_username: e.target.value }));
                  }}
                />
              </DrawerField>
              <DrawerField label="LinkedIn URL">
                <Input
                  value={form.linkedin_url}
                  onChange={(e) => {
                    setDirty(true);
                    setForm((f) => ({ ...f, linkedin_url: e.target.value }));
                  }}
                  placeholder="https://..."
                />
              </DrawerField>
            </div>
            <DrawerField label="Bio (Korean)">
              <MarkdownEditor
                slug={slug}
                extension="profile"
                value={form.bio_ko}
                onChange={(v) => {
                  setDirty(true);
                  setForm((f) => ({ ...f, bio_ko: v }));
                }}
              />
            </DrawerField>
            <DrawerField label="Bio (English)">
              <MarkdownEditor
                slug={slug}
                extension="profile"
                value={form.bio_en}
                onChange={(v) => {
                  setDirty(true);
                  setForm((f) => ({ ...f, bio_en: v }));
                }}
              />
            </DrawerField>

            {/* Education repeater */}
            <div className="border-t border-line pt-4 mt-4">
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-sm font-semibold">Education</h3>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    setDirty(true);
                    setForm((f) => ({
                      ...f,
                      education: [
                        ...f.education,
                        { institution: "", degree: "", field: "", start_year: "", end_year: "" },
                      ],
                    }));
                  }}
                >
                  <Plus size={12} /> Add
                </Button>
              </div>
              {form.education.map((row, i) => (
                <div key={i} className="space-y-2 mb-3 border border-line rounded p-3">
                  <div className="grid grid-cols-2 gap-2">
                    <Input
                      placeholder="Institution"
                      value={row.institution}
                      onChange={(e) => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          education: f.education.map((r, j) =>
                            j === i ? { ...r, institution: e.target.value } : r,
                          ),
                        }));
                      }}
                    />
                    <Input
                      placeholder="Degree"
                      value={row.degree}
                      onChange={(e) => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          education: f.education.map((r, j) =>
                            j === i ? { ...r, degree: e.target.value } : r,
                          ),
                        }));
                      }}
                    />
                  </div>
                  <div className="grid grid-cols-3 gap-2">
                    <Input
                      placeholder="Field"
                      value={row.field}
                      onChange={(e) => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          education: f.education.map((r, j) =>
                            j === i ? { ...r, field: e.target.value } : r,
                          ),
                        }));
                      }}
                    />
                    <Input
                      placeholder="Start year"
                      value={row.start_year}
                      onChange={(e) => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          education: f.education.map((r, j) =>
                            j === i ? { ...r, start_year: e.target.value } : r,
                          ),
                        }));
                      }}
                    />
                    <Input
                      placeholder="End year"
                      value={row.end_year}
                      onChange={(e) => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          education: f.education.map((r, j) =>
                            j === i ? { ...r, end_year: e.target.value } : r,
                          ),
                        }));
                      }}
                    />
                    {errors[`education.${i}`] && (
                      <p className="text-xs text-red-600 col-span-3">{errors[`education.${i}`]}</p>
                    )}
                  </div>
                  <div className="flex justify-end">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          education: f.education.filter((_, j) => j !== i),
                        }));
                      }}
                    >
                      <Trash2 size={12} /> Remove
                    </Button>
                  </div>
                </div>
              ))}
            </div>

            {/* Custom links repeater */}
            <div className="border-t border-line pt-4 mt-4">
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-sm font-semibold">Custom links</h3>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    setDirty(true);
                    setForm((f) => ({
                      ...f,
                      custom_links: [...f.custom_links, { label: "", url: "", icon: "" }],
                    }));
                  }}
                >
                  <Plus size={12} /> Add
                </Button>
              </div>
              {form.custom_links.map((row, i) => (
                <div key={i} className="grid grid-cols-3 gap-2 mb-2">
                  <Input
                    placeholder="Label"
                    value={row.label}
                    onChange={(e) => {
                      setDirty(true);
                      setForm((f) => ({
                        ...f,
                        custom_links: f.custom_links.map((r, j) =>
                          j === i ? { ...r, label: e.target.value } : r,
                        ),
                      }));
                    }}
                  />
                  <Input
                    placeholder="https://..."
                    value={row.url}
                    onChange={(e) => {
                      setDirty(true);
                      setForm((f) => ({
                        ...f,
                        custom_links: f.custom_links.map((r, j) =>
                          j === i ? { ...r, url: e.target.value } : r,
                        ),
                      }));
                    }}
                  />
                  <div className="flex gap-1">
                    <Input
                      placeholder="icon"
                      value={row.icon}
                      onChange={(e) => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          custom_links: f.custom_links.map((r, j) =>
                            j === i ? { ...r, icon: e.target.value } : r,
                          ),
                        }));
                      }}
                    />
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        setDirty(true);
                        setForm((f) => ({
                          ...f,
                          custom_links: f.custom_links.filter((_, j) => j !== i),
                        }));
                      }}
                    >
                      <Trash2 size={12} />
                    </Button>
                  </div>
                </div>
              ))}
            </div>

            {errors._form && <p className="text-sm text-red-600 mt-3">{errors._form}</p>}
          </div>
        }
        preview={
          <DraftPreviewPane>
            <ProfileViewCmp
              profile={{
                display_name: form.display_name || "Unnamed",
                tagline_ko: form.tagline_ko || null,
                tagline_en: form.tagline_en || null,
                avatar_url: form.avatar_url || null,
                bio_ko: form.bio_ko || null,
                bio_en: form.bio_en || null,
                email: form.email || null,
                github_username: form.github_username || null,
                linkedin_url: form.linkedin_url || null,
                education: form.education.map((r) => ({
                  institution: r.institution || null,
                  degree: r.degree || null,
                  field: r.field || null,
                  start_year: r.start_year ? Number(r.start_year) : null,
                  end_year: r.end_year ? Number(r.end_year) : null,
                })),
                custom_links: form.custom_links.map((r) => ({
                  label: r.label,
                  url: r.url,
                  icon: r.icon || null,
                })),
                updated_at: updatedAt,
              }}
              language="ko"
            />
          </DraftPreviewPane>
        }
        footer={
          staleNotice ? (
            <>
              <Button
                variant="outline"
                onClick={() => {
                  setStaleNotice(null);
                  load();
                }}
              >
                Reload remote
              </Button>
              <Button variant="outline" onClick={() => setStaleNotice(null)}>
                Keep mine
              </Button>
            </>
          ) : (
            <>
              <Button variant="outline" onClick={() => setOpen(false)} disabled={save.isPending}>
                Cancel
              </Button>
              <Button onClick={() => save.mutate()} disabled={save.isPending || !dirty}>
                {save.isPending ? "Saving..." : "Save"}
              </Button>
            </>
          )
        }
      />
    </div>
  );
}
