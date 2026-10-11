import type { Meeting, Workspace } from "./types";

export const initialWorkspaces: Workspace[] = [
  {
    "id": 1,
    "name": "Brand refresh",
    "description": "A clearer voice. A more considered identity.",
    "color": "pink",
    "symbol": "✳",
    "overview": "Reimagining our identity for the next chapter. Bringing strategy, design, and storytelling into one shared direction.",
    "decision": "Lead with the warm editorial direction. Keep the wordmark and simplify the supporting palette.",
    "task": "Present the revised identity system",
    "owner": "Lina",
    "due": "Oct 14"
  },
  {
    "id": 2,
    "name": "Product launch",
    "description": "Turning a promising idea into a thoughtful launch.",
    "color": "sage",
    "symbol": "↗",
    "overview": "Preparing the autumn release and making the first-run experience feel effortless.",
    "decision": "Launch the core experience on October 22. Move advanced integrations into November.",
    "task": "Finalize the launch checklist",
    "owner": "Omar",
    "due": "Oct 12"
  },
  {
    "id": 3,
    "name": "Studio operations",
    "description": "Small improvements to the way we work together.",
    "color": "lav",
    "symbol": "◈",
    "overview": "A shared home for team rituals, resource planning, and a calmer way of working.",
    "decision": "Protect Wednesday mornings for focus and move status updates to an asynchronous format.",
    "task": "Share the updated team calendar",
    "owner": "Sara",
    "due": "Oct 13"
  }
];

export const initialMeetings: Meeting[] = [
  {
    "id": 1,
    "title": "A new chapter for the brand",
    "project": 1,
    "date": "Oct 9, 2026",
    "duration": "42 min",
    "summary": "The team aligned on a warmer, more editorial brand direction. The identity should feel confident and human, with typography doing most of the storytelling.",
    "highlights": [
      "Reviewed three directions against audience expectations.",
      "Selected the warm editorial concept.",
      "Discussed a phased rollout across the website and communications."
    ],
    "decision": "Lead with the warm editorial direction. Keep the wordmark and simplify the supporting palette.",
    "actions": [
      "Present the revised identity system",
      "Collect feedback on typography"
    ],
    "speakers": [
      "Shahd",
      "Lina",
      "Daniel"
    ]
  },
  {
    "id": 2,
    "title": "Launch readiness check-in",
    "project": 2,
    "date": "Oct 8, 2026",
    "duration": "28 min",
    "summary": "The core product is on track for October 22. The team prioritized onboarding quality and a focused launch story over expanding the initial feature set.",
    "highlights": [
      "Confirmed the October 22 release date.",
      "Reviewed feedback from five pilot customers.",
      "Moved advanced integrations to November."
    ],
    "decision": "Launch the core experience on October 22. Move advanced integrations into November.",
    "actions": [
      "Finalize the launch checklist",
      "Review onboarding copy"
    ],
    "speakers": [
      "Shahd",
      "Omar",
      "Maya"
    ]
  },
  {
    "id": 3,
    "title": "Design direction workshop",
    "project": 1,
    "date": "Oct 6, 2026",
    "duration": "56 min",
    "summary": "Explored warm editorial, bold geometric, and minimal monochrome concepts. The first two were shortlisted, with an initial preference for the editorial route.",
    "highlights": [
      "Mapped audience expectations.",
      "Shortlisted editorial and geometric concepts.",
      "Requested a typography exploration for the next review."
    ],
    "decision": "Develop the editorial and geometric concepts for the next review.",
    "actions": [
      "Prepare the direction comparison"
    ],
    "speakers": [
      "Lina",
      "Daniel",
      "Shahd"
    ]
  },
  {
    "id": 4,
    "title": "A better rhythm for the team",
    "project": 3,
    "date": "Oct 5, 2026",
    "duration": "35 min",
    "summary": "The team agreed to reduce recurring meetings and protect a weekly focus block. A two-week trial will evaluate the impact on delivery.",
    "highlights": [
      "Identified fragmented focus time.",
      "Reserved Wednesday mornings for deep work.",
      "Agreed on written Monday updates."
    ],
    "decision": "Protect Wednesday mornings for focus and move status updates to an asynchronous format.",
    "actions": [
      "Share the updated team calendar"
    ],
    "speakers": [
      "Sara",
      "Shahd",
      "Omar"
    ]
  },
  {
    "id": 5,
    "title": "Customer discovery debrief",
    "project": 2,
    "date": "Oct 3, 2026",
    "duration": "48 min",
    "summary": "Pilot customers valued a clear first-run experience most. Research points to simpler onboarding and fewer decisions before the first useful result.",
    "highlights": [
      "Reviewed five interviews.",
      "Prioritized onboarding friction.",
      "Agreed to test a simplified welcome flow."
    ],
    "decision": "Test simpler onboarding with the pilot group.",
    "actions": [
      "Review onboarding copy"
    ],
    "speakers": [
      "Maya",
      "Omar",
      "Shahd"
    ]
  }
];
