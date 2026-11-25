import { useEffect } from "react";
import { Navigate } from "react-router";
import { useAuthSession } from "@/domain/auth";
import { useProfileAgreementStore } from "@/store/profileAgreementStore";
import RouteLoadingFallback from "./RouteLoadingFallback";

export default function EntryRoutePage() {
  const { session, status } = useAuthSession();
  const setChecked = useProfileAgreementStore((state) => state.setChecked);

  useEffect(() => {
    if (status === "authenticated" && session?.authenticated) setChecked(true);
  }, [session, setChecked, status]);

  if (status === "checking" || (status === "authenticated" && session?.authenticated)) {
    return status === "checking" ? <RouteLoadingFallback fullScreen /> : <AuthenticatedEntry />;
  }
  return <Navigate replace to="/profiles" />;
}

function AuthenticatedEntry() {
  const checked = useProfileAgreementStore((state) => state.checked);
  return checked ? <Navigate replace to="/workspace" /> : <RouteLoadingFallback fullScreen />;
}
