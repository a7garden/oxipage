import { useQuery } from "@tanstack/react-query";

import { fetchProfile, type Profile } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { ProfileView } from "./ProfileView";

export function ProfilePage() {
  const { lang } = useLanguage();
  const { data: profile, isLoading, error } = useQuery<Profile | null>({
    queryKey: ["profile"],
    queryFn: fetchProfile,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (error || !profile) {
    return (
      <p className="text-subtle">
        {lang === "ko" ? "프로필을 불러오지 못했습니다." : "Failed to load profile."}
      </p>
    );
  }

  return <ProfileView profile={profile} language={lang} />;
}