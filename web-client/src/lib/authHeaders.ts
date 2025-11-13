export const buildAuthHeaders = (authToken: string): Record<string, string> => {
    const headers: Record<string, string> = {
        "X-Moor-Auth-Token": authToken,
    };
    const clientToken = localStorage.getItem("client_token");
    const clientId = localStorage.getItem("client_id");

    if (clientToken) {
        headers["X-Moor-Client-Token"] = clientToken;
    }
    if (clientId) {
        headers["X-Moor-Client-Id"] = clientId;
    }

    return headers;
};
