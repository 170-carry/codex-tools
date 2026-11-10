import { createContext, useContext } from "react";
export const PresenceContext = createContext(false);
export const useExiting = () => useContext(PresenceContext);
