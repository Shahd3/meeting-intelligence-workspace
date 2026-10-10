export const destinations = ["Dashboard", "Meetings", "Projects", "Settings"] as const;
export type Destination = (typeof destinations)[number];
